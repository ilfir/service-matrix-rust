use std::{fs, sync::Arc};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use service_matrix_rust::{build_app, config::Config, storage::DictionaryStore};
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    _root: TempDir,
    app: Router,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let data = root.path().join("data");
        let resources = root.path().join("resources");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(&resources).unwrap();
        fs::write(
            resources.join("definitions.txt"),
            "ab\nabc\nжир\nexcluded\n",
        )
        .unwrap();
        fs::write(
            resources.join("merged.txt"),
            "alphabet\nsmall\nwith-hyphen\n",
        )
        .unwrap();
        fs::write(data.join("include.txt"), "included\n").unwrap();
        fs::write(data.join("exclude.txt"), "excluded\n").unwrap();
        let config = Config {
            port: 8080,
            data_dir: data.clone(),
            resources_dir: resources.clone(),
            environment: "Testing".into(),
            git_sha: "0123456789abcdef0123456789abcdef01234567".into(),
        };
        let store = Arc::new(DictionaryStore::load(data, resources).unwrap());
        Self {
            _root: root,
            app: build_app(config, store),
        }
    }

    async fn request(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(uri);
        let request_body = if let Some(body) = body {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_vec(&body).unwrap())
        } else {
            Body::empty()
        };
        let response = self
            .app
            .clone()
            .oneshot(builder.body(request_body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
        };
        (status, value)
    }
}

#[tokio::test]
async fn search_uses_csharp_defaults_and_response_shape() {
    let fixture = Fixture::new();
    let (status, body) = fixture
        .request(
            Method::POST,
            "/words/Search",
            Some(json!({"lettersMatrix":[["a","b"],["c","x"]]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["abc"].is_object());
    assert_eq!(body["abc"]["2"]["c"], "1 0");
}

#[tokio::test]
async fn search_supports_cyrillic() {
    let fixture = Fixture::new();
    let (status, body) = fixture
        .request(
            Method::POST,
            "/words/Search",
            Some(json!({
                "maxLength": 3,
                "minLength": 3,
                "maxWords": 5,
                "lettersMatrix":[["ж","и","р"]]
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["жир"].is_object());
}

#[tokio::test]
async fn search_accepts_empty_cells_as_blocked_positions() {
    let fixture = Fixture::new();
    let (status, body) = fixture
        .request(
            Method::POST,
            "/words/Search",
            Some(json!({
                "maxLength": 3,
                "minLength": 3,
                "maxWords": 5,
                "lettersMatrix":[["a", ""], ["x", "b"], ["x", "c"]]
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["abc"].is_object());
    assert_eq!(body["abc"]["1"]["b"], "1 1");
}

#[tokio::test]
async fn client_input_is_tolerant_like_aspnet_core() {
    let fixture = Fixture::new();
    let (status, body) = fixture
        .request(
            Method::POST,
            "/WORDS/search/",
            Some(json!({
                "MAXLENGTH": "3",
                "MaxWords": "5",
                "MINlength": "3",
                "LETTERSMATRIX":[["ж","и","р"]]
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["жир"].is_object());

    let (status, body) = fixture
        .request(Method::GET, "/WORDS/list?INCLUDE=false", None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!(["excluded"]));

    let (status, body) = fixture
        .request(
            Method::GET,
            "/WORDS/lookupword?WORD=AB&EXACTMATCH=true",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|item| item["word"] == "ab")
    );
}

#[tokio::test]
async fn search_rejects_invalid_and_malformed_requests() {
    let fixture = Fixture::new();
    let (status, body) = fixture
        .request(
            Method::POST,
            "/words/Search",
            Some(json!({"maxWords":0,"lettersMatrix":[["ab"],[]]})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], false);
    assert_eq!(body["error"], "Validation failed");
    assert!(body["details"].as_array().unwrap().len() >= 3);

    let request = Request::builder()
        .method(Method::POST)
        .uri("/words/Search")
        .header("content-type", "application/json")
        .body(Body::from("{"))
        .unwrap();
    let response = fixture.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_and_list_match_the_csharp_contract_and_refresh_state() {
    let fixture = Fixture::new();
    let (status, body) = fixture
        .request(
            Method::POST,
            "/words/Update",
            Some(json!({"words":["second","INCLUDED"],"include":true})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, 1);

    let (_, include) = fixture.request(Method::GET, "/words/List", None).await;
    assert_eq!(include.as_array().unwrap().len(), 2);
    let (_, exclude) = fixture
        .request(Method::GET, "/words/List?include=false", None)
        .await;
    assert_eq!(exclude, json!(["excluded"]));

    let (missing_status, missing_body) = fixture
        .request(Method::POST, "/words/Update", Some(json!({"include":true})))
        .await;
    assert_eq!(missing_status, StatusCode::BAD_REQUEST);
    assert_eq!(missing_body["success"], false);
}

#[tokio::test]
async fn merge_cleanup_and_lookup_have_stable_shapes() {
    let fixture = Fixture::new();
    let (merge_status, merge) = fixture.request(Method::POST, "/words/Merge", None).await;
    assert_eq!(merge_status, StatusCode::OK);
    assert_eq!(merge["addedCount"], 1);
    assert_eq!(merge["removedCount"], 1);

    let (lookup_status, lookup) = fixture
        .request(
            Method::GET,
            "/words/LookupWord?word=included&exactMatch=true",
            None,
        )
        .await;
    assert_eq!(lookup_status, StatusCode::OK);
    let locations: Vec<&str> = lookup
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["location"].as_str().unwrap())
        .collect();
    assert!(locations.contains(&"Merged"));
    assert!(locations.contains(&"Included"));

    let (clean_status, clean) = fixture
        .request(Method::GET, "/words/CleanMerge", None)
        .await;
    assert_eq!(clean_status, StatusCode::OK);
    assert_eq!(clean["success"], true);
    assert!(clean["message"].as_str().unwrap().contains("BEFORE:"));
    assert!(clean["message"].as_str().unwrap().contains("AFTER:"));
}

#[tokio::test]
async fn lookup_requires_a_nonblank_word() {
    let fixture = Fixture::new();
    for uri in [
        "/words/LookupWord",
        "/words/LookupWord?word=%20%20&exactMatch=false",
    ] {
        let (status, body) = fixture.request(Method::GET, uri, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body["error"],
            "Word parameter must be provided and cannot be empty."
        );
    }
}

#[tokio::test]
async fn version_health_openapi_and_swagger_are_available() {
    let fixture = Fixture::new();
    let (status, version) = fixture.request(Method::GET, "/version", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(version["success"], true);
    assert_eq!(version["data"]["environmentName"], "Testing");
    assert_eq!(
        version["data"]["sha"],
        "0123456789abcdef0123456789abcdef01234567"
    );

    let (health_status, health) = fixture.request(Method::GET, "/health", None).await;
    assert_eq!(health_status, StatusCode::OK);
    assert_eq!(health, json!({"status":"healthy"}));

    let (openapi_status, openapi) = fixture
        .request(Method::GET, "/swagger/v1/swagger.json", None)
        .await;
    assert_eq!(openapi_status, StatusCode::OK);
    assert!(openapi["paths"]["/words/Search"].is_object());

    let response = fixture
        .app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(response.status().is_success() || response.status().is_redirection());
    let _ = to_bytes(response.into_body(), usize::MAX).await.unwrap();
}

#[tokio::test]
async fn cors_preflight_and_concurrent_reads_succeed() {
    let fixture = Fixture::new();
    let response = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/words/List")
                .header("origin", "https://example.test")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["access-control-allow-origin"], "*");

    let tasks: Vec<_> = (0..20)
        .map(|_| {
            let app = fixture.app.clone();
            tokio::spawn(async move {
                app.oneshot(
                    Request::builder()
                        .uri("/words/List?include=true")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status()
            })
        })
        .collect();
    for task in tasks {
        assert_eq!(task.await.unwrap(), StatusCode::OK);
    }
}
