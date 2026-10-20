use thiserror::Error;

use crate::types::ProfileLease;
use crate::types_v1::RateLimit;

/// Errors from the client.
#[derive(Debug, Error)]
pub enum Error {
    /// A non-2xx response from the API.
    #[error("{message} ({kind}, HTTP {status})")]
    Api {
        status: u16,
        /// `error.type`: invalid_request, authentication, payment, not_found, conflict, rate_limit, api_error.
        kind: String,
        /// A finer reason, such as `profile_busy`, `session_limit` or `session_not_open`.
        code: Option<String>,
        param: Option<String>,
        message: String,
        /// With `profile_busy`: what uses the profile and when its lease next runs out.
        lease: Option<ProfileLease>,
        request_id: Option<String>,
        /// With a 429: the whole seconds the API said to wait (`Retry-After`).
        retry_after: Option<u64>,
        /// Your key's limit, when the response carried the `RateLimit-*` headers.
        rate_limit: Option<RateLimit>,
    },
    /// The request never got a response.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    /// An event stream kept closing before it sent anything.
    #[error("the event stream closed without sending anything")]
    StreamClosed,
    /// `wait` gave up before the intent settled.
    #[error("intent {intent_id} did not settle within {timeout:?}")]
    WaitTimeout {
        intent_id: String,
        timeout: std::time::Duration,
    },
    /// No API key was given and OPENINTENTS_API_KEY is unset.
    #[error("missing API key: pass one or set OPENINTENTS_API_KEY")]
    MissingApiKey,
    /// A webhook signature was missing, malformed, wrong or too old.
    #[error("webhook verification failed: {0}")]
    Webhook(&'static str),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
