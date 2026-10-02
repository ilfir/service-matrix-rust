use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::{Map, Value};
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

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub max_length: usize,
    pub max_words: usize,
    pub min_length: usize,
    pub letters_matrix: Option<Vec<Vec<String>>>,
}

impl<'de> Deserialize<'de> for SearchRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("search request must be a JSON object"))?;

        Ok(Self {
            max_length: deserialize_usize_field(object, "maxLength", default_max_length())?,
            max_words: deserialize_usize_field(object, "maxWords", default_max_words())?,
            min_length: deserialize_usize_field(object, "minLength", default_min_length())?,
            letters_matrix: deserialize_optional_field(object, "lettersMatrix")?,
        })
    }
}

fn field_case_insensitive<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    object
        .iter()
        .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value))
}

fn deserialize_usize_field<E>(
    object: &Map<String, Value>,
    name: &str,
    default: usize,
) -> Result<usize, E>
where
    E: serde::de::Error,
{
    let Some(value) = field_case_insensitive(object, name) else {
        return Ok(default);
    };
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .ok_or_else(|| E::custom(format!("{name} must be a non-negative integer"))),
        Value::String(number) => number
            .trim()
            .parse::<usize>()
            .map_err(|_| E::custom(format!("{name} must be an integer or numeric string"))),
        _ => Err(E::custom(format!(
            "{name} must be an integer or numeric string"
        ))),
    }
}

fn deserialize_optional_field<T, E>(object: &Map<String, Value>, name: &str) -> Result<Option<T>, E>
where
    T: serde::de::DeserializeOwned,
    E: serde::de::Error,
{
    match field_case_insensitive(object, name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(E::custom),
    }
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

#[derive(Clone, Debug, Default, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWordsRequest {
    pub words: Option<Vec<String>>,
    pub include: bool,
}

impl<'de> Deserialize<'de> for UpdateWordsRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("update request must be a JSON object"))?;
        let include = match field_case_insensitive(object, "include") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(Value::String(value)) if value.eq_ignore_ascii_case("true") => true,
            Some(Value::String(value)) if value.eq_ignore_ascii_case("false") => false,
            Some(_) => return Err(D::Error::custom("include must be a boolean")),
        };
        Ok(Self {
            words: deserialize_optional_field(object, "words")?,
            include,
        })
    }
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
    fn search_accepts_case_insensitive_properties_and_numeric_strings() {
        let request: SearchRequest = serde_json::from_str(
            r#"{"MAXLENGTH":"5","MaxWords":"10","MINlength":"1","LETTERSMATRIX":[["a"]]}"#,
        )
        .unwrap();
        assert_eq!(request.max_length, 5);
        assert_eq!(request.max_words, 10);
        assert_eq!(request.min_length, 1);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn update_accepts_case_insensitive_properties_and_boolean_strings() {
        let request: super::UpdateWordsRequest =
            serde_json::from_str(r#"{"WORDS":["test"],"INCLUDE":"TRUE"}"#).unwrap();
        assert_eq!(request.words.unwrap(), vec!["test"]);
        assert!(request.include);
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
