use crate::db::DatabaseError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    // 404 Error
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Database error")]
    DatabaseError(#[from] DatabaseError),
    // Internal Server Error
    #[error("Internal server error: {0}")]
    InternalServerError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // How we want errors responses to be serialized
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match self {
            AppError::NotFound(msg) => {
                // Add msg to not found message
                (StatusCode::NOT_FOUND, format!("Not found: {}", msg))
            }
            AppError::DatabaseError(_e) => {
                // Add error to database error message
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
            AppError::InternalServerError(msg) => {
                // Add msg to internal server error message
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Internal server error: {}", msg),
                )
            }
        };

        (status, Json(ErrorResponse { message })).into_response()
    }
}
