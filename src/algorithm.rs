use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap, HashSet},
};

use thiserror::Error;

use crate::dto::SearchResponse;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MatrixError {
    #[error("the matrix must contain at least one row")]
    Empty,
    #[error("matrix rows must be non-empty and rectangular")]
    Jagged,
    #[error("every non-empty matrix cell must contain exactly one character")]
    InvalidCell,
}

#[derive(Clone, Debug)]
pub struct MatrixIndex {
    originals: Vec<String>,
    normalized: Vec<String>,
    rows: usize,
    columns: usize,
    first_character_index: HashMap<String, Vec<usize>>,
    neighbors: Vec<Vec<usize>>,
    available_characters: HashSet<String>,
}

struct SearchState {
    visited: Vec<bool>,
    path: Vec<usize>,
    iterations: usize,
    iteration_limit: usize,
}

impl MatrixIndex {
    pub fn new(matrix: &[Vec<String>]) -> Result<Self, MatrixError> {
        let Some(first_row) = matrix.first() else {
            return Err(MatrixError::Empty);
        };
        let columns = first_row.len();
        if columns == 0 || matrix.iter().any(|row| row.len() != columns) {
            return Err(MatrixError::Jagged);
        }
        if matrix.iter().flatten().any(|cell| cell.chars().count() > 1) {
            return Err(MatrixError::InvalidCell);
        }

        let rows = matrix.len();
        let mut originals = Vec::with_capacity(rows * columns);
        let mut normalized = Vec::with_capacity(rows * columns);
        let mut first_character_index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut available_characters = HashSet::new();

        for (index, cell) in matrix.iter().flatten().enumerate() {
            let Some(character) = cell.chars().next() else {
                originals.push(String::new());
                normalized.push(String::new());
                continue;
            };
            let normalized_cell = normalize_character(character);
            originals.push(cell.clone());
            normalized.push(normalized_cell.clone());
            available_characters.insert(normalized_cell.clone());
            first_character_index
                .entry(normalized_cell)
                .or_default()
                .push(index);
        }

        let mut neighbors = Vec::with_capacity(rows * columns);
        for row in 0..rows {
            for column in 0..columns {
                let mut cell_neighbors = Vec::with_capacity(8);
                for row_delta in -1_isize..=1 {
                    for column_delta in -1_isize..=1 {
                        if row_delta == 0 && column_delta == 0 {
                            continue;
                        }
                        let next_row = row as isize + row_delta;
                        let next_column = column as isize + column_delta;
                        if next_row >= 0
                            && next_row < rows as isize
                            && next_column >= 0
                            && next_column < columns as isize
                        {
                            cell_neighbors.push(next_row as usize * columns + next_column as usize);
                        }
                    }
                }
                neighbors.push(cell_neighbors);
            }
        }

        Ok(Self {
            originals,
            normalized,
            rows,
            columns,
            first_character_index,
            neighbors,
            available_characters,
        })
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn contains_all_characters(&self, word: &str) -> bool {
        normalized_characters(word)
            .iter()
            .all(|character| self.available_characters.contains(character))
    }

    pub fn find_word(&self, word: &str) -> Option<Vec<Position>> {
        let characters = normalized_characters(word);
        let first = characters.first()?;
        if !characters
            .iter()
            .all(|character| self.available_characters.contains(character))
        {
            return None;
        }
        let starts = self.first_character_index.get(first)?;
        let mut state = SearchState {
            visited: vec![false; self.originals.len()],
            path: Vec::with_capacity(characters.len()),
            iterations: 0,
            iteration_limit: self
                .originals
                .len()
                .saturating_mul(characters.len().saturating_add(1))
                .saturating_mul(5),
        };

        for &start in starts {
            state.visited[start] = true;
            state.path.push(start);
            if self.backtrack(&characters, 1, start, &mut state) {
                return Some(
                    state
                        .path
                        .into_iter()
                        .map(|index| Position {
                            row: index / self.columns,
                            column: index % self.columns,
                        })
                        .collect(),
                );
            }
            state.path.clear();
            state.visited.fill(false);
        }
        None
    }

    fn backtrack(
        &self,
        word: &[String],
        word_index: usize,
        cell: usize,
        state: &mut SearchState,
    ) -> bool {
        state.iterations = state.iterations.saturating_add(1);
        if state.iterations > state.iteration_limit {
            return false;
        }
        if word_index >= word.len() {
            return true;
        }

        for &neighbor in &self.neighbors[cell] {
            if state.visited[neighbor] || self.normalized[neighbor] != word[word_index] {
                continue;
            }
            state.visited[neighbor] = true;
            state.path.push(neighbor);
            if self.backtrack(word, word_index + 1, neighbor, state) {
                return true;
            }
            state.path.pop();
            state.visited[neighbor] = false;
        }
        false
    }

    pub fn found_string(&self, path: &[Position]) -> String {
        path.iter()
            .map(|position| self.originals[position.row * self.columns + position.column].as_str())
            .collect()
    }

    pub fn path_response(&self, path: &[Position]) -> BTreeMap<usize, BTreeMap<String, String>> {
        path.iter()
            .enumerate()
            .map(|(index, position)| {
                let cell = self.originals[position.row * self.columns + position.column].clone();
                let location = format!("{} {}", position.row, position.column);
                (index, BTreeMap::from([(cell, location)]))
            })
            .collect()
    }
}

pub fn search_dictionary(
    matrix: &MatrixIndex,
    candidates: impl IntoIterator<Item = String>,
    min_length: usize,
    max_length: usize,
    max_words: usize,
) -> SearchResponse {
    let mut found = Vec::new();
    for word in candidates {
        let length = word.chars().count();
        if length < min_length || length > max_length || !matrix.contains_all_characters(&word) {
            continue;
        }
        if let Some(path) = matrix.find_word(&word) {
            found.push((word, matrix.path_response(&path)));
        }
    }

    found.sort_by_key(|(word, _)| Reverse(word.chars().count()));
    found.truncate(max_words);
    found.into_iter().collect()
}

pub(crate) fn normalize_word(word: &str) -> String {
    word.chars().flat_map(char::to_uppercase).collect()
}

fn normalize_character(character: char) -> String {
    character.to_uppercase().collect()
}

fn normalized_characters(word: &str) -> Vec<String> {
    word.chars().map(normalize_character).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter()
            .map(|row| row.iter().map(|cell| (*cell).to_owned()).collect())
            .collect()
    }

    #[test]
    fn validates_matrix_shape_and_cells() {
        assert!(matches!(MatrixIndex::new(&[]), Err(MatrixError::Empty)));
        assert!(matches!(
            MatrixIndex::new(&[vec![]]),
            Err(MatrixError::Jagged)
        ));
        assert!(matches!(
            MatrixIndex::new(&strings(&[&["a"], &["a", "b"]])),
            Err(MatrixError::Jagged)
        ));
        assert!(matches!(
            MatrixIndex::new(&strings(&[&["ab"]])),
            Err(MatrixError::InvalidCell)
        ));

        let matrix = MatrixIndex::new(&strings(&[&["a", ""], &["", "b"]])).unwrap();
        assert_eq!(
            matrix.find_word("ab"),
            Some(vec![
                Position { row: 0, column: 0 },
                Position { row: 1, column: 1 }
            ])
        );
        assert!(matrix.find_word("aa").is_none());
    }

    #[test]
    fn reports_dimensions_and_available_characters() {
        let matrix = MatrixIndex::new(&strings(&[&["a", "Б"], &["c", "d"]])).unwrap();
        assert_eq!((matrix.rows(), matrix.columns()), (2, 2));
        assert!(matrix.contains_all_characters("бad"));
        assert!(!matrix.contains_all_characters("z"));
    }

    #[test]
    fn finds_all_eight_directions_from_a_center_cell() {
        let matrix = MatrixIndex::new(&strings(&[
            &["1", "2", "3"],
            &["4", "x", "5"],
            &["6", "7", "8"],
        ]))
        .unwrap();
        for word in ["x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8"] {
            assert!(matrix.find_word(word).is_some(), "missing {word}");
        }
    }

    #[test]
    fn backtracks_and_never_reuses_a_cell() {
        let matrix = MatrixIndex::new(&strings(&[&["a", "b"], &["b", "c"]])).unwrap();
        assert!(matrix.find_word("abbc").is_some());
        assert!(matrix.find_word("aba").is_none());
    }

    #[test]
    fn tries_multiple_starts_and_preserves_original_case() {
        let matrix = MatrixIndex::new(&strings(&[
            &["A", "x", "x"],
            &["x", "x", "x"],
            &["a", "b", "C"],
        ]))
        .unwrap();
        let path = matrix.find_word("abc").unwrap();
        assert_eq!(matrix.found_string(&path), "abC");
        assert_eq!(path[0], Position { row: 2, column: 0 });
        let response = matrix.path_response(&path);
        assert_eq!(response[&2]["C"], "2 2");
    }

    #[test]
    fn handles_empty_and_missing_words() {
        let matrix = MatrixIndex::new(&strings(&[&["a"]])).unwrap();
        assert!(matrix.find_word("").is_none());
        assert!(matrix.find_word("z").is_none());
        assert!(matrix.find_word("aa").is_none());
    }

    #[test]
    fn searches_cyrillic_and_sorts_longest_first() {
        let matrix = MatrixIndex::new(&strings(&[
            &["ж", "и", "р", "x", "x"],
            &["x", "н", "е", "т", "ь"],
        ]))
        .unwrap();
        let response = search_dictionary(
            &matrix,
            vec!["жир".into(), "ЖИРНЕТЬ".into(), "none".into()],
            3,
            10,
            1,
        );
        assert_eq!(response.len(), 1);
        assert!(response.contains_key("ЖИРНЕТЬ"));
    }

    #[test]
    fn applies_length_bounds_and_preserves_source_order_for_ties() {
        let matrix = MatrixIndex::new(&strings(&[&["a", "b"], &["c", "d"]])).unwrap();
        let response = search_dictionary(
            &matrix,
            vec![
                "a".into(),
                "dc".into(),
                "ab".into(),
                "aba".into(),
                "abcdx".into(),
            ],
            2,
            4,
            10,
        );
        assert_eq!(
            response.keys().cloned().collect::<Vec<_>>(),
            vec!["dc", "ab"]
        );

        let case_tie = search_dictionary(&matrix, vec!["AB".into(), "ab".into()], 2, 2, 10);
        assert_eq!(
            case_tie.keys().cloned().collect::<Vec<_>>(),
            vec!["AB", "ab"]
        );
    }

    #[test]
    fn iteration_limit_terminates_pathological_search() {
        let matrix = MatrixIndex::new(&strings(&[
            &["a", "a", "a"],
            &["a", "a", "a"],
            &["a", "a", "a"],
        ]))
        .unwrap();
        assert!(matrix.find_word("aaaaaaaaaa").is_none());
    }

    #[test]
    fn unicode_case_normalization_matches_csharp_intent() {
        assert_eq!(normalize_word("абазин"), "АБАЗИН");
        assert_eq!(normalize_word("Rust"), "RUST");
    }
}
