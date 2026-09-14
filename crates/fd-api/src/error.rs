//! API errors.
//!
//! Every failure reaches the client as `{"error": "..."}` with a sentence in
//! it. The existing client reads that field and shows it verbatim, which is the
//! point: a status code alone tells a user nothing they can act on.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),

    /// The request was fine; the store simply has nothing yet. Distinct from a
    /// bad request because the fix is to run an ingest, not to change the call.
    #[error("{0}")]
    NoData(String),

    /// No such run, market or resource.
    #[error("{0}")]
    NotFound(String),

    /// The resource exists already (a paper run for that market and
    /// timeframe); stop it first.
    #[error("{0}")]
    Conflict(String),

    #[error("store: {0}")]
    Store(#[from] fd_store::StoreError),

    #[error("{0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NoData(_) | Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Store(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(serde_json::json!({ "error": self.to_string() }))).into_response()
    }
}
