//! Types for the OpenIntents v1 API. Source of truth: `spec/openapi.yaml`.
//! Unknown fields are ignored and unknown enum values map to `Unknown`, so new
//! API fields never break older SDK versions.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentStatus {
    Queued,
    Running,
    RequiresPayment,
    Completed,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}

impl IntentStatus {
    /// `completed`, `failed` and `cancelled` are final.
    pub fn is_final(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::RequiresPayment => "requires_payment",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }
}

/// Parameters for `POST /intents`. Build with [`CreateIntent::new`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateIntent {
    /// What to buy or book, in natural language.
    pub intent: String,
    /// Spending cap in cents; the intent fails rather than exceed it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Structured details: addresses, names, loyalty numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    /// Always return a payment link instead of paying from the balance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_payment_link: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

impl CreateIntent {
    pub fn new(intent: impl Into<String>) -> Self {
        Self {
            intent: intent.into(),
            ..Default::default()
        }
    }

    pub fn max_amount(mut self, cents: i64) -> Self {
        self.max_amount = Some(cents);
        self
    }

    pub fn currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = Some(currency.into());
        self
    }

    pub fn context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }

    pub fn require_payment_link(mut self, yes: bool) -> Self {
        self.require_payment_link = Some(yes);
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub reason: String,
    pub payment_url: String,
    #[serde(default)]
    pub merchant: Option<String>,
    pub total: i64,
    pub fee: i64,
    pub currency: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptItem {
    pub name: String,
    pub quantity: i64,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub merchant: String,
    #[serde(default)]
    pub items: Vec<ReceiptItem>,
    pub total: i64,
    pub fee: i64,
    pub currency: String,
    #[serde(default)]
    pub confirmation: Option<String>,
    #[serde(default)]
    pub receipt_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub id: String,
    pub status: IntentStatus,
    pub intent: String,
    #[serde(default)]
    pub max_amount: Option<i64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub context: Option<Value>,
    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
    /// Set when `status` is `requires_payment`.
    #[serde(default)]
    pub payment: Option<PaymentRequest>,
    /// Set when `status` is `completed`.
    #[serde(default)]
    pub result: Option<Receipt>,
    /// Set when `status` is `failed`.
    #[serde(default)]
    pub error: Option<IntentError>,
    #[serde(default)]
    pub test: bool,
    pub created_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ListIntents {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<IntentStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentList {
    pub data: Vec<Intent>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub available: i64,
    pub currency: String,
    #[serde(default)]
    pub monthly_limit: Option<i64>,
    #[serde(default)]
    pub spent_this_month: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub created_at: String,
    pub data: Intent,
}
