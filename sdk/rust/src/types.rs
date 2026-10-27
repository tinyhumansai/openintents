//! Types for the OpenIntents v1 API. Source of truth: `spec/openapi.yaml`.
//! Unknown fields are ignored and unknown enum values map to `Unknown`, so new
//! API fields never break older SDK versions.

use std::collections::HashMap;

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::types_v1::{SecretRequest, SignIn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentStatus {
    Queued,
    Running,
    RequiresPayment,
    /// Waits for the payer to answer what the agent asks while it signs them
    /// in to the shop (`input_request`).
    RequiresInput,
    /// Waits for the payer to sign in to the shop on `sign_in.url`.
    RequiresLogin,
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
            Self::RequiresInput => "requires_input",
            Self::RequiresLogin => "requires_login",
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
    /// Always wait in `requires_payment` with a payment link (`reason: requested`) before paying.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_payment_link: Option<bool>,
    /// When the shop wants the payer signed in and asks for a phone number or
    /// email and then a code: let the agent sign them in itself, asking for
    /// each value in `requires_input` (`true`, the default), or never
    /// (`false`). Takes effect only where OpenIntents has agent sign-in on
    /// for your key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_in_agent: Option<bool>,
    /// The payer pays the shop themselves (default `false`). At the payment
    /// step the intent waits in `requires_input` with an
    /// [`InputKind::Payment`] request whose `image_url` is a picture of the
    /// shop's payment code (a UPI QR code); answer with
    /// [`AnswerInput::done`] once they paid, or decline. No wallet is used
    /// and the fee is waived for now. Live keys only; where it is not available for your
    /// key, creating the intent is a 503 `pay_at_shop_unavailable`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pay_at_shop: Option<bool>,
    /// The payer's phone number or email for signing in, so the agent need
    /// not ask for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_in_hint: Option<SignInHint>,
    /// When the shop wants the payer signed in and the agent does not sign
    /// them in itself: wait in `requires_login` with a sign-in link for them
    /// (`true`, the default), or fail with `login_required` (`false`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_in_link: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    /// A customer of your account to buy for: their wallet and browser profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    /// The country its browser goes out from (ISO 3166-1 alpha-2, lowercase).
    /// Left out, the payer's last country, one inferred from `end_user_ip`,
    /// or the default is used; see `country_source` on the intent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// The address your end user is browsing from, to infer `country` from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_user_ip: Option<String>,
    /// Where this intent's own events go, signed with the `webhook_secret`
    /// the create response alone carries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    /// How long a question to the payer may wait for an answer, in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_ttl_seconds: Option<u32>,
    /// Handles (`sec_...`) of the stored secrets the purchase may use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Vec<String>>,
    /// The shops (registrable domains) you let the purchase fill secrets on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shops: Option<Vec<String>>,
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

    pub fn sign_in_agent(mut self, yes: bool) -> Self {
        self.sign_in_agent = Some(yes);
        self
    }

    pub fn pay_at_shop(mut self, yes: bool) -> Self {
        self.pay_at_shop = Some(yes);
        self
    }

    pub fn sign_in_hint(mut self, hint: SignInHint) -> Self {
        self.sign_in_hint = Some(hint);
        self
    }

    pub fn sign_in_link(mut self, yes: bool) -> Self {
        self.sign_in_link = Some(yes);
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
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

    pub fn webhook_url(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }

    pub fn input_ttl_seconds(mut self, seconds: u32) -> Self {
        self.input_ttl_seconds = Some(seconds);
        self
    }

    pub fn secrets<I, S>(mut self, handles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.secrets = Some(handles.into_iter().map(Into::into).collect());
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
}

/// Why a browser goes out from its country.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountrySource {
    /// You named it.
    Explicit,
    /// The country the payer last browsed from.
    Payer,
    /// Inferred from the `end_user_ip` you gave.
    EndUserIp,
    /// Inferred from the address the payer's own page was opened from.
    RequestIp,
    /// The default.
    Default,
    #[serde(other)]
    Unknown,
}

/// Where the payer signs in to a shop, so a waiting intent carries on. Send
/// `url` to the payer only: it works for 30 minutes, on the first device that
/// opens it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignInRequest {
    pub url: String,
    /// The shop's domain, such as `amazon.com`.
    pub shop: String,
    pub expires_at: String,
}

/// The payer's phone number or email for the agent to sign in with. Kept
/// sealed, used once, deleted when the intent ends, and never returned.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SignInHint {
    /// Digits, with an optional leading `+`; spaces, dashes, dots and
    /// parentheses are fine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl SignInHint {
    pub fn phone(mut self, phone: impl Into<String>) -> Self {
        self.phone = Some(phone.into());
        self
    }

    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }
}

/// What an [`InputRequest`] needs from the payer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    /// Their phone number, to sign in with.
    Phone,
    /// Their email, to sign in with.
    Email,
    /// A one-time code the shop sent them.
    Code,
    /// Their approval of the sign-in, in the shop's app or in a notification
    /// on another device. It has no value: answer with [`AnswerInput::done`].
    Approval,
    /// Anything else the purchase needs that the request did not give, such
    /// as the email the shop wants for the order, a size or a colour. The
    /// `prompt` is then the agent's own question (never asking for passwords,
    /// codes, PINs, card or bank details or ID numbers), and the answer goes
    /// back to the agent.
    Detail,
    /// An identity document (a passport for a flight, a licence for a
    /// rental). Answered only on the request's `url`, where the payer enters
    /// it; [`Client::answer_intent_input`](crate::Client::answer_intent_input)
    /// refuses any answer but a decline (`422 answer_on_page`).
    Sensitive,
    /// The payer pays the shop themselves (`pay_at_shop`): show them
    /// `image_url`, a picture of the shop's payment code to scan with a UPI
    /// app, and answer with [`AnswerInput::done`] once they paid (it has no
    /// value), or decline. It expires after 5 minutes.
    Payment,
    #[serde(other)]
    Unknown,
}

/// One of the choices a question offers, such as a seat, a fare or a delivery slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputChoice {
    /// What an answer names; unique in the question.
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    /// What choosing it adds to the price (negative, takes off), in minor
    /// units of `currency`.
    #[serde(default)]
    pub price_delta: Option<i64>,
    #[serde(default)]
    pub currency: Option<String>,
}

/// What a field of a form takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputFieldType {
    Text,
    Email,
    Phone,
    /// `YYYY-MM-DD`.
    Date,
    Number,
    /// One of the field's `options`, answered by the option's `id`.
    Choice,
    #[serde(other)]
    Unknown,
}

/// One field of a form a question asks the payer to fill in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputField {
    /// The key of this field's answer in `values`.
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: InputFieldType,
    pub required: bool,
    /// For a `choice` field, its options.
    #[serde(default)]
    pub options: Vec<InputChoice>,
}

/// What a plain answer to a question must look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputExpects {
    Text,
    Email,
    Phone,
    Choice,
    #[serde(other)]
    Unknown,
}

/// Where the shop sent a code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputChannel {
    Sms,
    Email,
    Call,
    App,
    #[serde(other)]
    Unknown,
}

/// What the agent needs from the payer while it signs them in to a shop
/// itself, or a [`InputKind::Detail`] the purchase needs (at most 3 per
/// intent). Ask them `prompt` and answer with
/// [`Client::answer_intent_input`](crate::Client::answer_intent_input) right
/// away: it expires after 5 minutes. Or send them `url` to type the answer
/// there.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRequest {
    /// Answer with this ID.
    pub id: String,
    pub kind: InputKind,
    /// The shop's registrable domain, such as `blinkit.com`.
    pub shop: String,
    /// What to ask the payer, written by OpenIntents (for a detail, the
    /// agent's own question). Show it to them as it is.
    pub prompt: String,
    /// A detail's choices (at most 8), empty otherwise. When not empty,
    /// answer with one of them (case-insensitive): anything else is a 400.
    #[serde(default)]
    pub options: Vec<String>,
    /// A detail that offers choices with ids, descriptions and price changes
    /// (2 to 50), empty otherwise. Answer with the `id` of one as the value:
    /// any other is a 422 `unknown_option`.
    #[serde(default)]
    pub choices: Vec<InputChoice>,
    /// A detail that is a form (at most 10 fields), empty otherwise. Answer
    /// every field at once with [`AnswerInput::values`].
    #[serde(default)]
    pub fields: Vec<InputField>,
    /// What a plain answer must look like.
    #[serde(default)]
    pub expects: Option<InputExpects>,
    /// Where the shop sent the code, when known.
    #[serde(default)]
    pub channel: Option<InputChannel>,
    /// How many characters the code has, when the page says.
    #[serde(default)]
    pub length: Option<u32>,
    /// A page where the payer types the answer themselves. Theirs only:
    /// whoever has it can answer.
    pub url: String,
    /// A [`InputKind::Payment`]'s picture (JPEG) of the shop's payment code,
    /// for the payer to scan; served only while the request waits, absent for
    /// other kinds. Like `url`, whoever has it can see it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// A [`InputKind::Payment`]'s UPI payment link (`upi://pay?…`) for the
    /// same payment as the code, when the shop's page gave one: open it in a
    /// UPI app on the phone instead of scanning. Absent for other kinds, and
    /// when the page gave none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upi_url: Option<String>,
    pub expires_at: String,
    /// How many more times the agent may ask for this kind in this purchase,
    /// after this one.
    pub attempts_left: u32,
}

/// An answer to an intent's `input_request`, for
/// [`Client::answer_intent_input`](crate::Client::answer_intent_input). Each
/// names the request by its `id`.
///
/// ```
/// use openintents::AnswerInput;
///
/// let code = AnswerInput::value("inp_8f2c41", "481516");
/// let no = AnswerInput::decline("inp_8f2c41");
/// # let _ = (code, no);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerInput {
    /// The phone number, email or code, or a detail's answer (one of its
    /// `options` when it has any).
    Value { input: String, value: String },
    /// Every field of a form at once, by the field's `name`.
    Values {
        input: String,
        values: HashMap<String, String>,
    },
    /// The payer would rather not: the agent stops signing them in this way.
    Decline { input: String },
    /// Only for an approval (the payer approved the sign-in) or a payment
    /// (the payer paid the shop).
    Done { input: String },
}

impl AnswerInput {
    pub fn value(input: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Value {
            input: input.into(),
            value: value.into(),
        }
    }

    pub fn values<I, K, V>(input: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self::Values {
            input: input.into(),
            values: values.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        }
    }

    pub fn decline(input: impl Into<String>) -> Self {
        Self::Decline { input: input.into() }
    }

    pub fn done(input: impl Into<String>) -> Self {
        Self::Done { input: input.into() }
    }
}

impl Serialize for AnswerInput {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        match self {
            Self::Value { input, value } => {
                map.serialize_entry("input", input)?;
                map.serialize_entry("value", value)?;
            }
            Self::Values { input, values } => {
                map.serialize_entry("input", input)?;
                map.serialize_entry("values", values)?;
            }
            Self::Decline { input } => {
                map.serialize_entry("input", input)?;
                map.serialize_entry("decline", &true)?;
            }
            Self::Done { input } => {
                map.serialize_entry("input", input)?;
                map.serialize_entry("done", &true)?;
            }
        }
        map.end()
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
    /// How the shop was paid: `wallet` (the payer's own wallet),
    /// `cash_on_delivery` or `upi_qr` (the payer themselves, `pay_at_shop`).
    /// Absent on older receipts and test purchases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paid_with: Option<String>,
    /// What was bought, where it is not an order of goods: `ride`, a ride
    /// requested from a ride app, its product as the item and its fare as
    /// the total. Absent on orders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// How far it got, where `kind` says: a `ride` is `requested`, the app
    /// looking for a driver, which none may have taken yet. Never a PIN or
    /// OTP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentError {
    /// Why the intent did not finish: on a `failed` one, such as
    /// `over_max_amount` or `needs_review`; on a `cancelled` one,
    /// `may_be_placed` (cancelled after its order was placed at the shop,
    /// which may have it).
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub id: String,
    pub status: IntentStatus,
    pub intent: String,
    /// The customer it buys for; `None` for your account's own intents.
    #[serde(default)]
    pub customer: Option<String>,
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
    /// Set when `status` is `requires_login`.
    #[serde(default)]
    pub sign_in: Option<SignInRequest>,
    /// Set when `status` is `requires_input`.
    #[serde(default)]
    pub input_request: Option<InputRequest>,
    /// Where this intent's own events go, if it was created with a `webhook_url`.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// Signs the `webhook_url` deliveries (`whsec_...`). Only in the create response.
    #[serde(default)]
    pub webhook_secret: Option<String>,
    /// The secrets the purchase may use, as given (never a value).
    #[serde(default)]
    pub secrets: Option<Vec<String>>,
    /// The shops you let the purchase fill secrets on.
    #[serde(default)]
    pub shops: Vec<String>,
    /// The country its browser goes out from.
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub country_source: Option<CountrySource>,
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
    /// Only intents bought for this customer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
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

/// Someone you buy for, with their own wallet and browser profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: String,
    /// Your own ID for them, unique in your account.
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// What the customer may spend; `None` fields are no limit.
    #[serde(default)]
    pub limits: Option<Limits>,
    pub created_at: String,
}

/// What a customer may spend, in cents, counting the cart's total of each
/// purchase their wallet paid. `None` is no limit. Checked before anything is
/// paid; an intent over one fails with `limit_exceeded`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    #[serde(default)]
    pub per_intent: Option<i64>,
    #[serde(default)]
    pub monthly: Option<i64>,
}

/// Limits to set on a customer. A field left out stays as it is; `Some(Some(n))`
/// sets it (1 to 1000000000); `Some(None)` removes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LimitsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_intent: Option<Option<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly: Option<Option<i64>>,
}

impl LimitsParams {
    pub fn per_intent(mut self, cents: Option<i64>) -> Self {
        self.per_intent = Some(cents);
        self
    }

    pub fn monthly(mut self, cents: Option<i64>) -> Self {
        self.monthly = Some(cents);
        self
    }
}

/// Fields for creating or updating a customer. On update, fields left out
/// stay as they are, an empty string clears `external_id`, `name` or
/// `email`, and `metadata` replaces the whole map.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CustomerParams {
    /// Unique in your account; one already in use is a 409 conflict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    /// What the customer may spend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<LimitsParams>,
}

impl CustomerParams {
    pub fn limits(mut self, limits: LimitsParams) -> Self {
        self.limits = Some(limits);
        self
    }

    pub fn external_id(mut self, id: impl Into<String>) -> Self {
        self.external_id = Some(id.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ListCustomers {
    /// Only the customer with this `external_id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerList {
    pub data: Vec<Customer>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// One step in an intent's or a session's timeline. `kind` (`type` in the
/// API) says what `data` holds: `status` (`{status, reason?}`), `step` and
/// `checkout` for intents, `page` and `command` for sessions. New kinds may
/// appear: skip the ones you don't know.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub created_at: String,
    #[serde(default)]
    pub data: Value,
}

/// A page of a timeline, oldest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventList {
    pub data: Vec<Event>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// Paging for [`Client::list_intent_events`](crate::Client::list_intent_events)
/// and [`Client::list_session_events`](crate::Client::list_session_events).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ListEvents {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    /// Nothing uses it.
    Ready,
    /// An intent or a session holds it (see `lease`).
    InUse,
    /// Being erased.
    Forgetting,
    /// Erased, and nothing saved in it since.
    Forgotten,
    #[serde(other)]
    Unknown,
}

/// What holds a profile: an intent or a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseHolder {
    /// `intent` or `session`.
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
}

/// What uses a profile now. A profile is used by one intent or session at a time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLease {
    pub holder: LeaseHolder,
    /// When the lease next runs out. It is renewed while the holder works.
    pub expires_at: String,
}

/// A payer's browser profile: the merchant logins, cookies and carts their
/// intents and sessions build up. Your account has one, and so does each
/// customer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    /// Its customer; `None` for your account's own profile.
    #[serde(default)]
    pub customer: Option<String>,
    pub status: ProfileStatus,
    #[serde(default)]
    pub lease: Option<ProfileLease>,
    /// The saved profile's size; `None` before anything is saved.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    pub created_at: String,
    /// When an intent or a session last took it.
    #[serde(default)]
    pub last_used_at: Option<String>,
}

/// Parameters for `POST /profiles`: your account's profile by default.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateProfile {
    /// This customer's profile instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
}

impl CreateProfile {
    pub fn customer(mut self, customer: impl Into<String>) -> Self {
        self.customer = Some(customer.into());
        self
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ListProfiles {
    /// Only this customer's profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileList {
    pub data: Vec<Profile>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Queued,
    Opening,
    Open,
    Closing,
    Closed,
    Failed,
    #[serde(other)]
    Unknown,
}

impl SessionStatus {
    /// `closed` and `failed` are final.
    pub fn is_final(self) -> bool {
        matches!(self, Self::Closed | Self::Failed)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Opening => "opening",
            Self::Open => "open",
            Self::Closing => "closing",
            Self::Closed => "closed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

/// Parameters for `POST /sessions`. The profile to open is named by
/// `profile` or by `customer`, not both; neither opens your account's own.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateSession {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    /// A public `https` URL to start on; without it, a blank page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
    /// 1 to 15 (the default), counted from when the session opens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_minutes: Option<u32>,
    /// The country its browser goes out from (ISO 3166-1 alpha-2, lowercase).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// The address your end user is browsing from, to infer `country` from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_user_ip: Option<String>,
    /// Where this session's own events go.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

impl CreateSession {
    pub fn country(mut self, country: impl Into<String>) -> Self {
        self.country = Some(country.into());
        self
    }

    pub fn end_user_ip(mut self, ip: impl Into<String>) -> Self {
        self.end_user_ip = Some(ip.into());
        self
    }

    pub fn webhook_url(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }

    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = Some(profile.into());
        self
    }

    pub fn customer(mut self, customer: impl Into<String>) -> Self {
        self.customer = Some(customer.into());
        self
    }

    pub fn start_url(mut self, url: impl Into<String>) -> Self {
        self.start_url = Some(url.into());
        self
    }

    pub fn max_minutes(mut self, minutes: u32) -> Self {
        self.max_minutes = Some(minutes);
        self
    }
}

/// A browser opened on a profile without an intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub profile: String,
    #[serde(default)]
    pub customer: Option<String>,
    pub status: SessionStatus,
    /// Opened with a test key.
    #[serde(default)]
    pub test: bool,
    #[serde(default)]
    pub start_url: Option<String>,
    /// The page it is on now.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub opened_at: Option<String>,
    /// When it closes by itself, at the latest.
    pub expires_at: String,
    #[serde(default)]
    pub closed_at: Option<String>,
    /// Why it ended: `released`, `idle`, `timed_out`, `data_limit`,
    /// `browser_lost` or `error`; others may appear.
    #[serde(default)]
    pub close_reason: Option<String>,
    /// The country its browser goes out from.
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub country_source: Option<CountrySource>,
    /// Where this session's own events go, if it was opened with a `webhook_url`.
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// Signs the `webhook_url` deliveries. Only in the create response.
    #[serde(default)]
    pub webhook_secret: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ListSessions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<SessionStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Only sessions on this customer's profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionList {
    pub data: Vec<Session>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// One thing to do on a session's page, for
/// [`Client::run_session_command`](crate::Client::run_session_command). A
/// session never pays or places an order: such commands come back `refused`.
///
/// ```
/// use openintents::SessionCommand;
///
/// let look = SessionCommand::snapshot();
/// let sign_in = SessionCommand::click("e12");
/// let email = SessionCommand::type_text("e7", "ada@example.com");
/// # let _ = (look, sign_in, email);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// Go to a public `https` URL; other hosts and ports are refused (`url_not_allowed`).
    Navigate { url: String },
    /// The page's accessibility tree, with a `ref` on each element you can act on.
    Snapshot,
    /// Act on an element by its `ref` (`element`) from the latest snapshot.
    /// After a navigate, and after any act but typing without line breaks,
    /// take a new snapshot before the next act.
    Act { element: String, action: ActAction },
    /// The page's readable text, or one element's with a CSS selector.
    Read { selector: Option<String> },
}

/// What a [`SessionCommand::Act`] does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActAction {
    Click,
    /// Type into a text field. Line breaks are fine, other control characters are not.
    Type(String),
    /// Pick this option.
    Select(String),
    /// One key, alone or with Shift or Control: `Enter`, `Shift+Tab`, `Control+a`.
    Press(String),
}

impl SessionCommand {
    pub fn navigate(url: impl Into<String>) -> Self {
        Self::Navigate { url: url.into() }
    }

    pub fn snapshot() -> Self {
        Self::Snapshot
    }

    /// The whole page's text.
    pub fn read() -> Self {
        Self::Read { selector: None }
    }

    /// One element's text, by CSS selector.
    pub fn read_selector(selector: impl Into<String>) -> Self {
        Self::Read {
            selector: Some(selector.into()),
        }
    }

    pub fn click(element: impl Into<String>) -> Self {
        Self::act(element, ActAction::Click)
    }

    pub fn type_text(element: impl Into<String>, text: impl Into<String>) -> Self {
        Self::act(element, ActAction::Type(text.into()))
    }

    pub fn select(element: impl Into<String>, value: impl Into<String>) -> Self {
        Self::act(element, ActAction::Select(value.into()))
    }

    pub fn press(element: impl Into<String>, key: impl Into<String>) -> Self {
        Self::act(element, ActAction::Press(key.into()))
    }

    fn act(element: impl Into<String>, action: ActAction) -> Self {
        Self::Act {
            element: element.into(),
            action,
        }
    }
}

impl Serialize for SessionCommand {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        match self {
            Self::Navigate { url } => {
                map.serialize_entry("type", "navigate")?;
                map.serialize_entry("url", url)?;
            }
            Self::Snapshot => map.serialize_entry("type", "snapshot")?,
            Self::Act { element, action } => {
                map.serialize_entry("type", "act")?;
                map.serialize_entry("ref", element)?;
                let (name, field) = match action {
                    ActAction::Click => ("click", None),
                    ActAction::Type(text) => ("type", Some(("text", text))),
                    ActAction::Select(value) => ("select", Some(("value", value))),
                    ActAction::Press(key) => ("press", Some(("key", key))),
                };
                map.serialize_entry("action", name)?;
                if let Some((key, value)) = field {
                    map.serialize_entry(key, value)?;
                }
            }
            Self::Read { selector } => {
                map.serialize_entry("type", "read")?;
                if let Some(selector) = selector {
                    map.serialize_entry("selector", selector)?;
                }
            }
        }
        map.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionCommandStatus {
    /// Still running: fetch it again by its `id`.
    Pending,
    Done,
    /// Not allowed, such as anything that would pay (`payment_not_allowed`).
    Refused,
    Failed,
    #[serde(other)]
    Unknown,
}

/// Why a session command was refused or failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failure {
    /// Such as `payment_not_allowed`, `url_not_allowed`, `not_found`,
    /// `not_actionable` or `session_closed`.
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCommandResult {
    pub id: String,
    /// The command's `type`: `navigate`, `snapshot`, `act` or `read`.
    #[serde(rename = "type")]
    pub kind: String,
    pub status: SessionCommandStatus,
    /// The page after the command.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    /// For a snapshot, the tree. It never includes what text fields hold.
    #[serde(default)]
    pub snapshot: Option<String>,
    /// For a read, the text.
    #[serde(default)]
    pub text: Option<String>,
    /// Why it was refused or failed.
    #[serde(default)]
    pub error: Option<Failure>,
    pub created_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
}

/// A webhook delivery. Dispatch on `kind` (`type` in the API), or match on
/// `data`: `intent.*` events carry an intent, `session.*` events (sent only to
/// endpoints subscribed to them) a session, `sign_in.*` a sign-in link and
/// `secret_request.completed` a secret request. Others (such as
/// `webhook_endpoint.test`) are [`WebhookData::Other`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawWebhookEvent")]
pub struct WebhookEvent {
    pub id: String,
    /// Such as `intent.completed` or `session.closed`.
    #[serde(rename = "type")]
    pub kind: String,
    pub created_at: String,
    pub data: WebhookData,
}

/// What a [`WebhookEvent`] is about.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum WebhookData {
    Intent(Box<Intent>),
    Session(Box<Session>),
    SignIn(Box<SignIn>),
    SecretRequest(Box<SecretRequest>),
    /// An event type this SDK does not know yet.
    Other(Value),
}

impl WebhookEvent {
    /// The intent, for `intent.*` events.
    pub fn intent(&self) -> Option<&Intent> {
        match &self.data {
            WebhookData::Intent(intent) => Some(intent),
            _ => None,
        }
    }

    /// The session, for `session.*` events.
    pub fn session(&self) -> Option<&Session> {
        match &self.data {
            WebhookData::Session(session) => Some(session),
            _ => None,
        }
    }

    /// The sign-in link, for `sign_in.*` events.
    pub fn sign_in(&self) -> Option<&SignIn> {
        match &self.data {
            WebhookData::SignIn(sign_in) => Some(sign_in),
            _ => None,
        }
    }

    /// The secret request, for `secret_request.completed`.
    pub fn secret_request(&self) -> Option<&SecretRequest> {
        match &self.data {
            WebhookData::SecretRequest(request) => Some(request),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct RawWebhookEvent {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    created_at: String,
    #[serde(default)]
    data: Value,
}

impl TryFrom<RawWebhookEvent> for WebhookEvent {
    type Error = serde_json::Error;

    fn try_from(raw: RawWebhookEvent) -> Result<Self, Self::Error> {
        let data = if raw.kind.starts_with("intent.") {
            WebhookData::Intent(Box::new(serde_json::from_value(raw.data)?))
        } else if raw.kind.starts_with("session.") {
            WebhookData::Session(Box::new(serde_json::from_value(raw.data)?))
        } else if raw.kind.starts_with("sign_in.") {
            WebhookData::SignIn(Box::new(serde_json::from_value(raw.data)?))
        } else if raw.kind.starts_with("secret_request.") {
            WebhookData::SecretRequest(Box::new(serde_json::from_value(raw.data)?))
        } else {
            WebhookData::Other(raw.data)
        };
        Ok(Self {
            id: raw.id,
            kind: raw.kind,
            created_at: raw.created_at,
            data,
        })
    }
}
