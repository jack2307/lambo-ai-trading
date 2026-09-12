//! Ingest errors.

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("http {status} from {url}")]
    Http { url: String, status: u16 },

    #[error("request: {0}")]
    Request(#[from] reqwest::Error),

    /// The venue answered, and said no. Distinct from a transport failure
    /// because retrying will not help.
    #[error("{0}")]
    Venue(String),
}
