//! A rate-limited HTTP client.
//!
//! Both venues publish their market data without a key and both will ban an IP
//! that hammers them. The delay is enforced here rather than at each call site
//! so that no future endpoint can forget it.

use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::error::IngestError;

const USER_AGENT: &str = concat!("flowdesk/", env!("CARGO_PKG_VERSION"), " (research)");

#[derive(Debug, Clone)]
pub struct Http {
    client: reqwest::Client,
    min_delay: Duration,
    /// Shared so that cloning the client does not clone away the rate limit.
    last_call: Arc<Mutex<Option<Instant>>>,
}

impl Http {
    #[must_use]
    pub fn new(min_delay: Duration) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("a default reqwest client always builds");
        Self { client, min_delay, last_call: Arc::new(Mutex::new(None)) }
    }

    /// GET and deserialize, waiting out the rate limit first.
    ///
    /// The lock is held across the wait on purpose: it is what serialises
    /// concurrent callers into the same delay rather than letting them all
    /// wait the same interval and then fire together.
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, IngestError> {
        let mut last = self.last_call.lock().await;
        if let Some(previous) = *last {
            let elapsed = previous.elapsed();
            if elapsed < self.min_delay {
                tokio::time::sleep(self.min_delay - elapsed).await;
            }
        }
        *last = Some(Instant::now());
        drop(last);

        let response = self.client.get(url).header("accept", "application/json").send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(IngestError::Http { url: url.to_string(), status: status.as_u16() });
        }
        Ok(response.json().await?)
    }
}
