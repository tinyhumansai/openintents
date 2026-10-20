//! Portal sessions, secrets, sign-ins, live links and webhook endpoints.
//! Source of truth: `spec/openapi.yaml`. Like [`crate::types`], unknown fields
//! are ignored and unknown enum values map to `Unknown`.

use serde::{Deserialize, Serialize};

use crate::types::CountrySource;

/// Your key's limit, from the `RateLimit-*` headers of a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    /// Requests a minute your key may make; also the burst.
    pub limit: u32,
    /// Requests left in your bucket.
    pub remaining: u32,
    /// Seconds until your bucket is full again.
    pub reset: u32,
}

/// Parameters for [`Client::create_portal_session`](crate::Client::create_portal_session).
#[derive(Debug, Clone, Default, Serialize)]
pub struct CreatePortalSession {
    /// Where the customer's page sends them back to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_user_ip: Option<String>,
    /// How long the link works, in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u32>,
}

impl CreatePortalSession {
    pub fn return_url(mut self, url: impl Into<String>) -> Self {
        self.return_url = Some(url.into());
        self
    }

    pub fn country(mut self, country: impl Into<String>) -> Self {
        self.country = Some(country.into());
        self
    }

    pub fn end_user_ip(mut self, ip: impl Into<String>) -> Self {
        self.end_user_ip = Some(ip.into());
        self
    }

    pub fn ttl_seconds(mut self, seconds: u32) -> Self {
        self.ttl_seconds = Some(seconds);
        self
    }
}

/// A link to a customer's own page, shown once. Send `url` to the customer only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalSession {
    pub id: String,
    pub customer: String,
    /// The country the customer's browser goes out from.
    pub country: String,
    pub country_source: CountrySource,
    /// Works once, until `expires_at`.
    pub url: String,
    pub expires_at: String,
}

/// What a secret holds, and so which fields it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKind {
    Address,
    Contact,
    IdentityDocument,
    ShopPassword,
    Loyalty,
    #[serde(other)]
    Unknown,
}

/// A secret to store: details a purchase may need and that you would not put
/// in a prompt. Build with [`CreateSecret::new`].
#[derive(Debug, Clone, Serialize)]
pub struct CreateSecret {
    /// The customer it belongs to; left out, your account's own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    pub kind: SecretKind,
    pub label: String,
    /// The values, by the field names its kind takes.
    pub fields: std::collections::HashMap<String, String>,
    /// The shops (registrable domains) it may be filled on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shops: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

impl CreateSecret {
    pub fn new<I, K, V>(kind: SecretKind, label: impl Into<String>, fields: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            customer: None,
            kind,
            label: label.into(),
            fields: fields.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
            shops: None,
            default: None,
            expires_at: None,
        }
    }

    pub fn customer(mut self, customer: impl Into<String>) -> Self {
        self.customer = Some(customer.into());
        self
    }

    pub fn shops<I, S>(mut self, shops: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.shops = Some(shops.into_iter().map(Into::into).collect());
        self
    }

    pub fn default(mut self, yes: bool) -> Self {
        self.default = Some(yes);
        self
    }

    pub fn expires_at(mut self, at: impl Into<String>) -> Self {
        self.expires_at = Some(at.into());
        self
    }
}

/// Whether a key sees test or live things.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Test,
    Live,
    #[serde(other)]
    Unknown,
}

/// A stored secret as you may see it: never its values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    pub id: String,
    /// The customer it belongs to; `None` for your account's own.
    #[serde(default)]
    pub customer: Option<String>,
    pub kind: SecretKind,
    pub label: String,
    /// What a person may be shown to recognise it, such as `Paris, FR` for an address.
    pub mask: String,
    /// The few fields its kind keeps in the clear.
    #[serde(default)]
    pub public: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub shops: Vec<String>,
    #[serde(default)]
    pub default: bool,
    /// The mode of the key that wrote it. A key sees only secrets of its own mode.
    pub mode: Mode,
    /// `developer` through this API, `payer` on their own page.
    pub entered_by: String,
    /// Goes up each time it is replaced.
    pub version: u32,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<String>,
}

/// Filters for [`Client::list_secrets`](crate::Client::list_secrets).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ListSecrets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<SecretKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretList {
    pub data: Vec<Secret>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// One row of a secret's history. Never a value or a mask.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretUse {
    pub id: String,
    pub secret: String,
    pub kind: SecretKind,
    /// `create`, `replace`, `delete`, `expire`, `fill`, `fund` or `refuse`; others may appear.
    pub action: String,
    /// `worker`, `api`, `payer`, `developer` or `system`.
    pub actor: String,
    /// `ok` or `refused`.
    pub outcome: String,
    /// Why a use was refused: `shop_not_allowed`, `expired` or `wrong_use`.
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub field: Option<String>,
    /// The names of the fields a create or replace set.
    #[serde(default)]
    pub field_names: Vec<String>,
    #[serde(default)]
    pub shop: Option<String>,
    #[serde(default)]
    pub intent: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretUseList {
    pub data: Vec<SecretUse>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// Paging for lists that take only a `limit` and a `cursor`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Page {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Parameters for [`Client::create_secret_request`](crate::Client::create_secret_request).
#[derive(Debug, Clone, Serialize)]
pub struct CreateSecretRequest {
    /// The kinds of secret the customer is asked for.
    pub kinds: Vec<SecretKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u32>,
}

impl CreateSecretRequest {
    pub fn new(kinds: impl IntoIterator<Item = SecretKind>) -> Self {
        Self {
            kinds: kinds.into_iter().collect(),
            return_url: None,
            ttl_seconds: None,
        }
    }

    pub fn return_url(mut self, url: impl Into<String>) -> Self {
        self.return_url = Some(url.into());
        self
    }

    pub fn ttl_seconds(mut self, seconds: u32) -> Self {
        self.ttl_seconds = Some(seconds);
        self
    }
}

/// A request for a customer's secrets, with the link to send them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRequestCreated {
    pub id: String,
    pub customer: String,
    pub kinds: Vec<SecretKind>,
    /// For the customer only. Works once, until `expires_at`.
    pub url: String,
    pub expires_at: String,
}

/// Where a request for a customer's secrets stands; also the data of
/// `secret_request.completed`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRequest {
    pub id: String,
    pub customer: String,
    pub mode: Mode,
    pub kinds: Vec<SecretKind>,
    /// `pending` until the customer fills it in, then `done`; `expired` if they did not in time.
    pub state: String,
    pub expires_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    /// The handles (`sec_...`) of the secrets the customer made.
    #[serde(default)]
    pub secrets: Vec<String>,
}

/// A link to a session's live view, shown once. Whoever holds `url` sees the
/// browser and drives it, though never to pay or place an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveLink {
    pub session: String,
    pub url: String,
    /// 10 minutes after it was made.
    pub expires_at: String,
}

/// Parameters for [`Client::create_sign_in`](crate::Client::create_sign_in).
#[derive(Debug, Clone, Serialize)]
pub struct CreateSignIn {
    /// A page of the shop the payer signs in to.
    pub shop_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_user_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
}

impl CreateSignIn {
    pub fn new(shop_url: impl Into<String>) -> Self {
        Self {
            shop_url: shop_url.into(),
            customer: None,
            country: None,
            end_user_ip: None,
            return_url: None,
        }
    }

    pub fn customer(mut self, customer: impl Into<String>) -> Self {
        self.customer = Some(customer.into());
        self
    }

    pub fn country(mut self, country: impl Into<String>) -> Self {
        self.country = Some(country.into());
        self
    }

    pub fn end_user_ip(mut self, ip: impl Into<String>) -> Self {
        self.end_user_ip = Some(ip.into());
        self
    }

    pub fn return_url(mut self, url: impl Into<String>) -> Self {
        self.return_url = Some(url.into());
        self
    }
}

/// Where a sign-in link stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignInStatus {
    /// Until the payer opens it.
    Open,
    /// While their browser is open.
    Active,
    /// They signed in.
    Done,
    Declined,
    Expired,
    Cancelled,
    #[serde(other)]
    Unknown,
}

/// A link where a payer signs in to a shop with no intent attached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignIn {
    pub id: String,
    pub status: SignInStatus,
    #[serde(default)]
    pub customer: Option<String>,
    /// The shop's domain, such as `amazon.com`.
    pub shop: String,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub country_source: Option<CountrySource>,
    #[serde(default)]
    pub return_url: Option<String>,
    /// The page for the payer only. Not in webhooks.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub test: bool,
    pub created_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
}

/// Parameters for [`Client::create_webhook_endpoint`](crate::Client::create_webhook_endpoint).
#[derive(Debug, Clone, Serialize)]
pub struct CreateWebhookEndpoint {
    pub url: String,
    /// The event types it receives. Empty or left out: every intent event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
}

impl CreateWebhookEndpoint {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            events: None,
        }
    }

    pub fn events<I, S>(mut self, events: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.events = Some(events.into_iter().map(Into::into).collect());
        self
    }
}

/// Changes to a webhook endpoint; fields left out stay as they are.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateWebhookEndpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// An empty list is every intent event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

impl UpdateWebhookEndpoint {
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn events<I, S>(mut self, events: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.events = Some(events.into_iter().map(Into::into).collect());
        self
    }

    pub fn disabled(mut self, yes: bool) -> Self {
        self.disabled = Some(yes);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: String,
    pub url: String,
    /// The event types it receives. Empty: every intent event.
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub disabled: bool,
    pub created_at: String,
    /// Present while the secret before the last rotation still signs as well.
    #[serde(default)]
    pub previous_secret_expires_at: Option<String>,
    /// The signing secret (`whsec_...`). Only in the create and rotate responses.
    #[serde(default)]
    pub secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpointList {
    pub data: Vec<WebhookEndpoint>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// How a webhook delivery stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    /// While it is being tried (and retried).
    Pending,
    Delivered,
    Failed,
    #[serde(other)]
    Unknown,
}

/// One send of an event to an endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    pub event_id: String,
    pub event_type: String,
    pub status: DeliveryStatus,
    pub attempts: u32,
    /// What your receiver answered on the last attempt, if it answered.
    #[serde(default)]
    pub response_code: Option<u16>,
    /// Why the last attempt failed, if it got no answer.
    #[serde(default)]
    pub error: Option<String>,
    pub created_at: String,
    /// The next attempt. Only while pending.
    #[serde(default)]
    pub next_attempt_at: Option<String>,
    #[serde(default)]
    pub delivered_at: Option<String>,
    /// The delivery this one sends again.
    #[serde(default)]
    pub redelivery_of: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDeliveryList {
    pub data: Vec<WebhookDelivery>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}
