use std::{
    cmp::Reverse,
    collections::HashSet,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use tempfile::NamedTempFile;
use thiserror::Error;

use crate::{
    algorithm::normalize_word,
    dto::{LookupResultResponseItem, MergeResponse},
};

#[derive(Clone, Debug)]
pub struct DictionarySnapshot {
    pub definitions: Arc<Vec<String>>,
    pub merged: Arc<Vec<String>>,
    pub include: Arc<Vec<String>>,
    pub exclude: Arc<Vec<String>>,
}

impl DictionarySnapshot {
    pub fn search_candidates(&self, min_length: usize, max_length: usize) -> Vec<String> {
        let excluded: HashSet<String> = self
            .exclude
            .iter()
            .map(|word| normalize_word(word))
            .collect();
        let mut seen = HashSet::new();
        self.definitions
            .iter()
            .chain(self.merged.iter())
            .filter(|word| {
                let length = word.chars().count();
                let normalized = normalize_word(word);
                length >= min_length
                    && length <= max_length
                    && !excluded.contains(&normalized)
                    && seen.insert(normalized)
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("required dictionary file is unavailable: {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to persist dictionary file: {path}: {message}")]
    Write { path: PathBuf, message: String },
    #[error("dictionary state lock was poisoned")]
    Lock,
}

#[derive(Debug)]
pub struct DictionaryStore {
    data_dir: PathBuf,
    resources_dir: PathBuf,
    state: RwLock<DictionarySnapshot>,
    mutation: Mutex<()>,
}

impl DictionaryStore {
    pub fn load(
        data_dir: impl Into<PathBuf>,
        resources_dir: impl Into<PathBuf>,
    ) -> Result<Self, StorageError> {
        let data_dir = data_dir.into();
        let resources_dir = resources_dir.into();
        let snapshot = load_snapshot(&data_dir, &resources_dir)?;
        Ok(Self {
            data_dir,
            resources_dir,
            state: RwLock::new(snapshot),
            mutation: Mutex::new(()),
        })
    }

    pub fn snapshot(&self) -> Result<DictionarySnapshot, StorageError> {
        self.state
            .read()
            .map(|state| state.clone())
            .map_err(|_| StorageError::Lock)
    }

    pub fn update_words(&self, words: &[String], include: bool) -> Result<usize, StorageError> {
        let _mutation = self.mutation.lock().map_err(|_| StorageError::Lock)?;
        let snapshot = self.snapshot()?;
        let mut target = if include {
            snapshot.include.as_ref().clone()
        } else {
            snapshot.exclude.as_ref().clone()
        };
        let mut seen: HashSet<String> = target.iter().map(|word| normalize_word(word)).collect();
        let mut added = 0;
        for word in words {
            if seen.insert(normalize_word(word)) {
                target.push(word.clone());
                added += 1;
            }
        }
        let path = self.data_dir.join(if include {
            "include.txt"
        } else {
            "exclude.txt"
        });
        atomic_write_lines(&path, &target)?;
        self.reload()?;
        Ok(added)
    }

    pub fn merge(&self) -> Result<MergeResponse, StorageError> {
        let _mutation = self.mutation.lock().map_err(|_| StorageError::Lock)?;
        let snapshot = self.snapshot()?;
        let mut output = Vec::new();
        let mut seen = HashSet::new();
        for word in snapshot.definitions.iter().chain(snapshot.merged.iter()) {
            if seen.insert(normalize_word(word)) {
                output.push(word.clone());
            }
        }
        let mut added_count = 0;
        for word in snapshot.include.iter() {
            if seen.insert(normalize_word(word)) {
                output.push(word.clone());
                added_count += 1;
            }
        }
        let excluded: HashSet<String> = snapshot
            .exclude
            .iter()
            .map(|word| normalize_word(word))
            .collect();
        let before = output.len();
        output.retain(|word| !excluded.contains(&normalize_word(word)));
        let removed_count = before - output.len();

        atomic_write_lines(&self.resources_dir.join("merged.txt"), &output)?;
        self.reload()?;
        Ok(MergeResponse {
            added_count,
            removed_count,
        })
    }

    pub fn clean_merge(&self) -> Result<(usize, usize), StorageError> {
        let _mutation = self.mutation.lock().map_err(|_| StorageError::Lock)?;
        let snapshot = self.snapshot()?;
        let before = snapshot.merged.len();
        let mut output: Vec<String> = snapshot
            .merged
            .iter()
            .filter(|word| {
                let length = word.chars().count();
                (8..=24).contains(&length) && !word.contains(' ') && !word.contains('-')
            })
            .cloned()
            .collect();
        output.sort_by_key(|word| Reverse(word.chars().count()));
        let after = output.len();
        atomic_write_lines(&self.resources_dir.join("merged.txt"), &output)?;
        self.reload()?;
        Ok((before, after))
    }

    pub fn lookup(
        &self,
        query: &str,
        exact_match: bool,
    ) -> Result<Vec<LookupResultResponseItem>, StorageError> {
        let snapshot = self.snapshot()?;
        let normalized_query = normalize_word(query);
        let mut results = Vec::new();
        for (words, location) in [
            (snapshot.definitions.as_ref(), "Dictionary"),
            (snapshot.merged.as_ref(), "Merged"),
            (snapshot.include.as_ref(), "Included"),
            (snapshot.exclude.as_ref(), "Excluded"),
        ] {
            for word in words {
                let normalized = normalize_word(word);
                let matches = if exact_match {
                    normalized == normalized_query
                } else {
                    normalized.contains(&normalized_query)
                };
                if matches {
                    results.push(LookupResultResponseItem {
                        word: word.clone(),
                        location: location.to_owned(),
                    });
                }
            }
        }
        Ok(results)
    }

    fn reload(&self) -> Result<(), StorageError> {
        let snapshot = load_snapshot(&self.data_dir, &self.resources_dir)?;
        let mut state = self.state.write().map_err(|_| StorageError::Lock)?;
        *state = snapshot;
        Ok(())
    }
}

fn load_snapshot(
    data_dir: &Path,
    resources_dir: &Path,
) -> Result<DictionarySnapshot, StorageError> {
    Ok(DictionarySnapshot {
        definitions: Arc::new(read_lines(&resources_dir.join("definitions.txt"))?),
        merged: Arc::new(read_lines(&resources_dir.join("merged.txt"))?),
        include: Arc::new(read_lines(&data_dir.join("include.txt"))?),
        exclude: Arc::new(read_lines(&data_dir.join("exclude.txt"))?),
    })
}

fn read_lines(path: &Path) -> Result<Vec<String>, StorageError> {
    let contents = fs::read_to_string(path).map_err(|source| StorageError::Read {
        path: path.to_owned(),
        source,
    })?;
    Ok(contents.lines().map(str::to_owned).collect())
}

fn atomic_write_lines(path: &Path, lines: &[String]) -> Result<(), StorageError> {
    let parent = path.parent().ok_or_else(|| StorageError::Write {
        path: path.to_owned(),
        message: "target has no parent directory".to_owned(),
    })?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|error| StorageError::Write {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        for line in lines {
            writeln!(writer, "{line}").map_err(|error| StorageError::Write {
                path: path.to_owned(),
                message: error.to_string(),
            })?;
        }
        writer.flush().map_err(|error| StorageError::Write {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| StorageError::Write {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    temporary
        .persist(path)
        .map_err(|error| StorageError::Write {
            path: path.to_owned(),
            message: error.error.to_string(),
        })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| StorageError::Write {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn store() -> (TempDir, DictionaryStore) {
        let root = TempDir::new().unwrap();
        let data = root.path().join("data");
        let resources = root.path().join("resources");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(&resources).unwrap();
        fs::write(resources.join("definitions.txt"), "alpha\nbeta\n").unwrap();
        fs::write(
            resources.join("merged.txt"),
            "alphabet\nshort\nwith-hyphen\n",
        )
        .unwrap();
        fs::write(data.join("include.txt"), "included\n").unwrap();
        fs::write(data.join("exclude.txt"), "beta\n").unwrap();
        let store = DictionaryStore::load(&data, &resources).unwrap();
        (root, store)
    }

    #[test]
    fn startup_requires_all_files() {
        let root = TempDir::new().unwrap();
        let error = DictionaryStore::load(root.path(), root.path()).unwrap_err();
        assert!(matches!(error, StorageError::Read { .. }));
    }

    #[test]
    fn candidates_are_filtered_excluded_and_deduplicated() {
        let (_root, store) = store();
        let candidates = store.snapshot().unwrap().search_candidates(4, 8);
        assert_eq!(candidates, vec!["alpha", "alphabet", "short"]);
    }

    #[test]
    fn update_is_case_insensitive_persistent_and_immediately_visible() {
        let (_root, store) = store();
        let added = store
            .update_words(&["INCLUDED".into(), "second".into()], true)
            .unwrap();
        assert_eq!(added, 1);
        assert_eq!(store.snapshot().unwrap().include.len(), 2);
        assert_eq!(store.update_words(&[], false).unwrap(), 0);
    }

    #[test]
    fn merge_adds_includes_removes_excludes_and_reports_counts() {
        let (_root, store) = store();
        let response = store.merge().unwrap();
        assert_eq!(response.added_count, 1);
        assert_eq!(response.removed_count, 1);
        let merged = &store.snapshot().unwrap().merged;
        assert!(merged.contains(&"included".to_owned()));
        assert!(!merged.contains(&"beta".to_owned()));
    }

    #[test]
    fn clean_merge_filters_sorts_persists_and_refreshes() {
        let (_root, store) = store();
        let (before, after) = store.clean_merge().unwrap();
        assert_eq!((before, after), (3, 1));
        assert_eq!(store.snapshot().unwrap().merged.as_ref(), &["alphabet"]);
    }

    #[test]
    fn lookup_reports_each_real_source() {
        let (_root, store) = store();
        let partial = store.lookup("alpha", false).unwrap();
        assert_eq!(partial.len(), 2);
        assert_eq!(partial[0].location, "Dictionary");
        assert_eq!(partial[1].location, "Merged");
        let exact = store.lookup("included", true).unwrap();
        assert_eq!(exact[0].location, "Included");
        assert!(store.lookup("missing", true).unwrap().is_empty());
    }

    #[test]
    fn atomic_write_rejects_parentless_target() {
        let error = atomic_write_lines(Path::new(""), &[]).unwrap_err();
        assert!(matches!(error, StorageError::Write { .. }));
    }
}
