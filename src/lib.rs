pub mod algorithm;
pub mod api;
pub mod config;
pub mod dto;
pub mod error;
pub mod storage;

use std::sync::Arc;

use axum::{Router, http::Method};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{api::AppState, config::Config, storage::DictionaryStore};

#[derive(OpenApi)]
#[openapi(
    paths(
        api::search,
        api::update_words,
        api::list_words,
        api::merge_words,
        api::clean_merge,
        api::lookup_word,
        api::version,
        api::health
    ),
    components(schemas(
        dto::SearchRequest,
        dto::UpdateWordsRequest,
        dto::MergeResponse,
        dto::LookupResultResponseItem,
        dto::ErrorResponse,
        dto::CleanMergeResponse,
        dto::VersionEnvelope,
        dto::VersionData,
        dto::HealthResponse
    )),
    tags((name = "Service Matrix", description = "C#-compatible word matrix API"))
)]
pub struct ApiDoc;

pub fn build_app(config: Config, store: Arc<DictionaryStore>) -> Router {
    let state = AppState { config, store };
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS, Method::HEAD])
        .allow_headers(Any);

    api::router()
        .merge(SwaggerUi::new("/").url("/swagger/v1/swagger.json", ApiDoc::openapi()))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}
