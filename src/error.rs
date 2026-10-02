use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::error;

use crate::{dto::ErrorResponse, storage::StorageError};

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    details: Option<Vec<String>>,
    internal: Option<String>,
}

impl ApiError {
    pub fn validation(message: impl Into<String>, details: Option<Vec<String>>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            details,
            internal: None,
        }
    }

    pub fn internal(message: impl Into<String>, internal: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            details: None,
            internal: Some(internal.into()),
        }
    }
}

impl From<StorageError> for ApiError {
    fn from(error: StorageError) -> Self {
        Self::internal("An unexpected storage error occurred.", error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let Some(internal) = &self.internal {
            error!(error = %internal, "request failed");
        }
        (
            self.status,
            Json(ErrorResponse {
                success: false,
                error: self.message,
                details: self.details,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use super::*;

    #[tokio::test]
    async fn validation_error_has_csharp_envelope() {
        let response =
            ApiError::validation("Validation failed", Some(vec!["bad".into()])).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"success":false,"error":"Validation failed","details":["bad"]})
        );
    }
}
