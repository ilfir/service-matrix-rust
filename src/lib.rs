pub mod algorithm;
pub mod api;
pub mod config;
pub mod dto;
pub mod error;
pub mod storage;

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, Uri},
    middleware::{self, Next},
    response::Response,
};
use tower::Layer;
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

    let app = api::router()
        .merge(SwaggerUi::new("/").url("/swagger/v1/swagger.json", ApiDoc::openapi()))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    Router::new().fallback_service(middleware::from_fn(normalize_client_uri).layer(app))
}

async fn normalize_client_uri(mut request: Request<Body>, next: Next) -> Response {
    let original = request.uri();
    let trimmed_path = if original.path() == "/" {
        "/"
    } else {
        original.path().trim_end_matches('/')
    };
    let canonical_path = match trimmed_path.to_ascii_lowercase().as_str() {
        "/words/search" => "/words/Search",
        "/words/update" => "/words/Update",
        "/words/list" => "/words/List",
        "/words/merge" => "/words/Merge",
        "/words/cleanmerge" => "/words/CleanMerge",
        "/words/lookupword" => "/words/LookupWord",
        "/version" => "/version",
        "/health" => "/health",
        "/swagger/v1/swagger.json" => "/swagger/v1/swagger.json",
        "/" => "/",
        _ => original.path(),
    };
    let canonical_query = original.query().map(normalize_query_names);
    let path_and_query = match canonical_query {
        Some(query) => format!("{canonical_path}?{query}"),
        None => canonical_path.to_owned(),
    };

    let mut parts = original.clone().into_parts();
    if let Ok(parsed) = path_and_query.parse() {
        parts.path_and_query = Some(parsed);
        if let Ok(uri) = Uri::from_parts(parts) {
            *request.uri_mut() = uri;
        }
    }
    next.run(request).await
}

fn normalize_query_names(query: &str) -> String {
    query
        .split('&')
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let canonical = if name.eq_ignore_ascii_case("include") {
                "include"
            } else if name.eq_ignore_ascii_case("word") {
                "word"
            } else if name.eq_ignore_ascii_case("exactMatch") {
                "exactMatch"
            } else {
                name
            };
            if pair.contains('=') {
                format!("{canonical}={value}")
            } else {
                canonical.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}
