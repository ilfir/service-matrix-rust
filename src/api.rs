use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State, rejection::JsonRejection},
    routing::{get, post},
};

use crate::{
    algorithm::{MatrixIndex, search_dictionary},
    config::Config,
    dto::{
        CleanMergeResponse, HealthResponse, ListQuery, LookupQuery, LookupResultResponseItem,
        MergeResponse, SearchRequest, SearchResponse, UpdateWordsRequest, VersionData,
        VersionEnvelope,
    },
    error::ApiError,
    storage::DictionaryStore,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub store: Arc<DictionaryStore>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/words/Search", post(search))
        .route("/words/Update", post(update_words))
        .route("/words/List", get(list_words))
        .route("/words/Merge", post(merge_words))
        .route("/words/CleanMerge", get(clean_merge))
        .route("/words/LookupWord", get(lookup_word))
        .route("/version", get(version))
        .route("/health", get(health))
}

fn json_payload<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    payload.map(|Json(value)| value).map_err(|rejection| {
        ApiError::validation("Validation failed", Some(vec![rejection.body_text()]))
    })
}

#[utoipa::path(
    post,
    path = "/words/Search",
    request_body = SearchRequest,
    responses(
        (status = 200, description = "Words and matrix paths found successfully"),
        (status = 400, description = "Invalid search request", body = crate::dto::ErrorResponse),
        (status = 500, description = "Unexpected search failure", body = crate::dto::ErrorResponse)
    ),
    tag = "Service Matrix"
)]
pub async fn search(
    State(state): State<AppState>,
    payload: Result<Json<SearchRequest>, JsonRejection>,
) -> Result<Json<SearchResponse>, ApiError> {
    let request = json_payload(payload)?;
    if let Err(details) = request.validate() {
        return Err(ApiError::validation("Validation failed", Some(details)));
    }
    let matrix = request.letters_matrix.as_ref().expect("validated matrix");
    let index = MatrixIndex::new(matrix).map_err(|error| {
        ApiError::validation("Validation failed", Some(vec![error.to_string()]))
    })?;
    let candidates = state
        .store
        .snapshot()?
        .search_candidates(request.min_length, request.max_length);
    let min_length = request.min_length;
    let max_length = request.max_length;
    let max_words = request.max_words;
    let result = tokio::task::spawn_blocking(move || {
        search_dictionary(&index, candidates, min_length, max_length, max_words)
    })
    .await
    .map_err(|error| {
        ApiError::internal(
            "An unexpected error occurred during word search.",
            error.to_string(),
        )
    })?;
    Ok(Json(result))
}

#[utoipa::path(
    post,
    path = "/words/Update",
    request_body = UpdateWordsRequest,
    responses(
        (status = 200, description = "Number of newly added words", body = i32),
        (status = 400, description = "Invalid update request", body = crate::dto::ErrorResponse)
    ),
    tag = "Service Matrix"
)]
pub async fn update_words(
    State(state): State<AppState>,
    payload: Result<Json<UpdateWordsRequest>, JsonRejection>,
) -> Result<Json<usize>, ApiError> {
    let request = json_payload(payload)?;
    let words = request.words.ok_or_else(|| {
        ApiError::validation(
            "Validation failed",
            Some(vec!["Words list is required.".into()]),
        )
    })?;
    Ok(Json(state.store.update_words(&words, request.include)?))
}

#[utoipa::path(
    get,
    path = "/words/List",
    params(("include" = Option<bool>, Query, description = "Return include or exclude words")),
    responses((status = 200, description = "Requested word list", body = Vec<String>)),
    tag = "Service Matrix"
)]
pub async fn list_words(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<String>>, ApiError> {
    let snapshot = state.store.snapshot()?;
    let words = if query.include {
        snapshot.include.as_ref().clone()
    } else {
        snapshot.exclude.as_ref().clone()
    };
    Ok(Json(words))
}

#[utoipa::path(
    post,
    path = "/words/Merge",
    responses((status = 200, description = "Dictionary merge counts", body = MergeResponse)),
    tag = "Service Matrix"
)]
pub async fn merge_words(State(state): State<AppState>) -> Result<Json<MergeResponse>, ApiError> {
    Ok(Json(state.store.merge()?))
}

#[utoipa::path(
    get,
    path = "/words/CleanMerge",
    responses((status = 200, description = "Merged dictionary cleanup result", body = crate::dto::CleanMergeResponse)),
    tag = "Service Matrix"
)]
pub async fn clean_merge(
    State(state): State<AppState>,
) -> Result<Json<CleanMergeResponse>, ApiError> {
    let (before, after) = state.store.clean_merge()?;
    Ok(Json(CleanMergeResponse {
        success: true,
        message: format!("BEFORE: {before} words, AFTER: {after} words."),
    }))
}

#[utoipa::path(
    get,
    path = "/words/LookupWord",
    params(
        ("word" = String, Query, description = "Word or substring to find"),
        ("exactMatch" = Option<bool>, Query, description = "Require an exact match")
    ),
    responses(
        (status = 200, description = "Dictionary lookup results", body = Vec<LookupResultResponseItem>),
        (status = 400, description = "Missing lookup word", body = crate::dto::ErrorResponse)
    ),
    tag = "Service Matrix"
)]
pub async fn lookup_word(
    State(state): State<AppState>,
    Query(query): Query<LookupQuery>,
) -> Result<Json<Vec<LookupResultResponseItem>>, ApiError> {
    let word = query
        .word
        .filter(|word| !word.trim().is_empty())
        .ok_or_else(|| {
            ApiError::validation("Word parameter must be provided and cannot be empty.", None)
        })?;
    Ok(Json(state.store.lookup(&word, query.exact_match)?))
}

#[utoipa::path(
    get,
    path = "/version",
    responses((status = 200, description = "Build and environment information", body = VersionEnvelope)),
    tag = "Service Matrix"
)]
pub async fn version(State(state): State<AppState>) -> Json<VersionEnvelope> {
    Json(VersionEnvelope {
        success: true,
        data: VersionData {
            sha: state.config.git_sha.clone(),
            framework_description: format!("Rust/Axum {}", env!("CARGO_PKG_VERSION")),
            environment_name: state.config.environment.clone(),
        },
    })
}

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Dictionary store is loaded", body = HealthResponse)),
    tag = "Service Matrix"
)]
pub async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    state.store.snapshot()?;
    Ok(Json(HealthResponse {
        status: "healthy".to_owned(),
    }))
}
