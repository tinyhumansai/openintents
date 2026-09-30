use std::time::{Duration, Instant};

use rand::Rng;
use reqwest::{header, Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::types::{Balance, CreateIntent, Intent, IntentList, IntentStatus, ListIntents};

pub const DEFAULT_BASE_URL: &str = "https://api.openintents.io/v1";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Client for OpenIntents: let your agents buy anything.
///
/// ```no_run
/// # async fn run() -> openintents::Result<()> {
/// use openintents::{Client, CreateIntent};
///
/// let oi = Client::from_env()?; // OPENINTENTS_API_KEY
/// let intent = oi.create_intent(CreateIntent::new("flat white, oat milk").max_amount(1000)).await?;
/// let intent = oi.wait(&intent.id, Default::default()).await?;
/// if let Some(payment) = &intent.payment {
///     println!("pay here: {}", payment.payment_url);
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    max_retries: u32,
}

/// Options for [`Client::wait`].
#[derive(Debug, Clone, Copy)]
pub struct WaitOptions {
    /// Give up after this long. Default 5 minutes.
    pub timeout: Duration,
    /// Time between polls. Default 2s.
    pub interval: Duration,
    /// Return when the intent needs payment (default true), so a person can
    /// be sent `payment.payment_url`. False keeps waiting until it is final.
    pub return_on_payment: bool,
}

impl Default for WaitOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            interval: Duration::from_secs(2),
            return_on_payment: true,
        }
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    error: Option<ErrorDetail>,
}

#[derive(Deserialize)]
struct ErrorDetail {
    #[serde(rename = "type")]
    kind: Option<String>,
    code: Option<String>,
    message: Option<String>,
    param: Option<String>,
}

impl Client {
    /// A client with the given API key and default settings.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder(api_key).build()
    }

    /// Reads OPENINTENTS_API_KEY (and OPENINTENTS_BASE_URL, if set).
    pub fn from_env() -> Result<Self> {
        let key = std::env::var("OPENINTENTS_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .ok_or(Error::MissingApiKey)?;
        let mut builder = Self::builder(key);
        if let Ok(url) = std::env::var("OPENINTENTS_BASE_URL") {
            builder = builder.base_url(url);
        }
        Ok(builder.build())
    }

    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 2,
        }
    }

    /// Start an intent. Returns immediately, usually with status `queued`.
    /// Sends a fresh Idempotency-Key; use [`Client::create_intent_idempotent`] to set your own.
    pub async fn create_intent(&self, params: CreateIntent) -> Result<Intent> {
        self.create_intent_idempotent(params, &uuid::Uuid::new_v4().to_string())
            .await
    }

    pub async fn create_intent_idempotent(&self, params: CreateIntent, idempotency_key: &str) -> Result<Intent> {
        self.send(Method::POST, "/intents", Some(idempotency_key), |r| r.json(&params))
            .await
    }

    pub async fn get_intent(&self, id: &str) -> Result<Intent> {
        self.send(Method::GET, &format!("/intents/{id}"), None, |r| r).await
    }

    pub async fn list_intents(&self, params: &ListIntents) -> Result<IntentList> {
        self.send(Method::GET, "/intents", None, |r| r.query(params)).await
    }

    /// Cancel an intent that hasn't paid yet. Cancelled intents are free.
    pub async fn cancel_intent(&self, id: &str) -> Result<Intent> {
        let key = uuid::Uuid::new_v4().to_string();
        self.send(Method::POST, &format!("/intents/{id}/cancel"), Some(&key), |r| r)
            .await
    }

    /// The agent balance intents pay from, and this month's spend.
    pub async fn balance(&self) -> Result<Balance> {
        self.send(Method::GET, "/balance", None, |r| r).await
    }

    /// Poll until the intent is final, or (by default) needs payment.
    pub async fn wait(&self, id: &str, opts: WaitOptions) -> Result<Intent> {
        let deadline = Instant::now() + opts.timeout;
        loop {
            let intent = self.get_intent(id).await?;
            if intent.status.is_final() || (opts.return_on_payment && intent.status == IntentStatus::RequiresPayment) {
                return Ok(intent);
            }
            if Instant::now() + opts.interval > deadline {
                return Err(Error::WaitTimeout {
                    intent_id: id.to_string(),
                    timeout: opts.timeout,
                });
            }
            tokio::time::sleep(opts.interval).await;
        }
    }

    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        idempotency_key: Option<&str>,
        build: impl Fn(RequestBuilder) -> RequestBuilder,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut attempt = 0;
        loop {
            let mut req = self
                .http
                .request(method.clone(), &url)
                .bearer_auth(&self.api_key)
                .header(header::ACCEPT, "application/json");
            if let Some(key) = idempotency_key {
                req = req.header("Idempotency-Key", key);
            }
            let res = match build(req).send().await {
                Ok(res) => res,
                Err(err) if attempt < self.max_retries && (err.is_connect() || err.is_timeout()) => {
                    tokio::time::sleep(backoff(attempt, None)).await;
                    attempt += 1;
                    continue;
                }
                Err(err) => return Err(err.into()),
            };
            let status = res.status();
            if status.is_success() {
                return Ok(res.json().await?);
            }
            if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < self.max_retries {
                let retry_after = res
                    .headers()
                    .get(header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<f64>().ok());
                tokio::time::sleep(backoff(attempt, retry_after)).await;
                attempt += 1;
                continue;
            }
            let request_id = res
                .headers()
                .get("request-id")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let detail = res.json::<ErrorBody>().await.ok().and_then(|b| b.error);
            return Err(Error::Api {
                status: status.as_u16(),
                kind: detail.as_ref().and_then(|d| d.kind.clone()).unwrap_or_else(|| {
                    if status.is_server_error() {
                        "api_error"
                    } else {
                        "invalid_request"
                    }
                    .into()
                }),
                code: detail.as_ref().and_then(|d| d.code.clone()),
                param: detail.as_ref().and_then(|d| d.param.clone()),
                message: detail
                    .and_then(|d| d.message)
                    .unwrap_or_else(|| format!("HTTP {}", status.as_u16())),
                request_id,
            });
        }
    }
}

fn backoff(attempt: u32, retry_after: Option<f64>) -> Duration {
    if let Some(secs) = retry_after.filter(|s| *s > 0.0) {
        return Duration::from_secs_f64(secs);
    }
    let base = (0.5 * 2f64.powi(attempt as i32)).min(8.0);
    Duration::from_secs_f64(base * rand::thread_rng().gen_range(0.75..1.25))
}

/// Configures a [`Client`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    api_key: String,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
}

impl ClientBuilder {
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_string();
        self
    }

    /// Per-request timeout. Default 30s.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Retries for 429, 5xx and connection errors. Default 2.
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    pub fn build(self) -> Client {
        let http = reqwest::Client::builder()
            .timeout(self.timeout)
            .user_agent(format!("openintents-rust/{VERSION}"))
            .build()
            .expect("reqwest client with rustls builds");
        Client {
            http,
            api_key: self.api_key,
            base_url: self.base_url,
            max_retries: self.max_retries,
        }
    }
}
