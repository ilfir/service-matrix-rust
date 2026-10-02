use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub type SearchResponse = IndexMap<String, BTreeMap<usize, BTreeMap<String, String>>>;

const fn default_max_length() -> usize {
    5
}

const fn default_max_words() -> usize {
    10
}

const fn default_min_length() -> usize {
    1
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    #[serde(default = "default_max_length")]
    pub max_length: usize,
    #[serde(default = "default_max_words")]
    pub max_words: usize,
    #[serde(default = "default_min_length")]
    pub min_length: usize,
    pub letters_matrix: Option<Vec<Vec<String>>>,
}

impl SearchRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if !(1..=100).contains(&self.max_length) {
            errors.push("MaxLength must be between 1 and 100.".to_owned());
        }
        if !(1..=100).contains(&self.min_length) {
            errors.push("MinLength must be between 1 and 100.".to_owned());
        }
        if self.max_words == 0 {
            errors.push("MaxWords must be at least 1.".to_owned());
        }

        let Some(matrix) = self.letters_matrix.as_ref() else {
            errors.push("LettersMatrix must be provided and cannot be empty.".to_owned());
            return Err(errors);
        };
        if matrix.is_empty() {
            errors.push("LettersMatrix must be provided and cannot be empty.".to_owned());
            return Err(errors);
        }
        let width = matrix[0].len();
        if width == 0 || matrix.iter().any(Vec::is_empty) {
            errors
                .push("Each row in LettersMatrix must be provided and cannot be empty.".to_owned());
        }
        if matrix.iter().any(|row| row.len() != width) {
            errors.push("All rows in LettersMatrix must have the same length.".to_owned());
        }
        if matrix
            .iter()
            .flatten()
            .any(|cell| cell.chars().count() != 1)
        {
            errors.push("Each LettersMatrix cell must contain exactly one character.".to_owned());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWordsRequest {
    pub words: Option<Vec<String>>,
    #[serde(default)]
    pub include: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    #[serde(default = "default_true")]
    pub include: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupQuery {
    pub word: Option<String>,
    #[serde(default)]
    pub exact_match: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MergeResponse {
    pub added_count: usize,
    pub removed_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LookupResultResponseItem {
    pub word: String,
    pub location: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CleanMergeResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionEnvelope {
    pub success: bool,
    pub data: VersionData,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionData {
    pub sha: String,
    pub framework_description: String,
    pub environment_name: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::SearchRequest;

    #[test]
    fn search_defaults_match_csharp() {
        let request: SearchRequest = serde_json::from_str(r#"{"lettersMatrix":[["a"]]}"#).unwrap();
        assert_eq!(request.max_length, 5);
        assert_eq!(request.max_words, 10);
        assert_eq!(request.min_length, 1);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn validation_reports_all_scalar_errors() {
        let request = SearchRequest {
            max_length: 0,
            max_words: 0,
            min_length: 101,
            letters_matrix: None,
        };
        let errors = request.validate().unwrap_err();
        assert_eq!(errors.len(), 4);
    }

    #[test]
    fn validation_rejects_empty_jagged_and_multi_character_cells() {
        let empty = SearchRequest {
            max_length: 5,
            max_words: 10,
            min_length: 1,
            letters_matrix: Some(vec![]),
        };
        assert!(empty.validate().is_err());

        let invalid = SearchRequest {
            letters_matrix: Some(vec![vec!["ab".into()], vec![]]),
            ..empty
        };
        let errors = invalid.validate().unwrap_err();
        assert_eq!(errors.len(), 3);
    }
}
