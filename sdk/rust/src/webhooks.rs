//! Verify OpenIntents webhook signatures.
//!
//! The `OpenIntents-Signature` header is `t=<unix seconds>,v1=<hex>`, where v1
//! is HMAC-SHA256 of `"{t}.{raw body}"` keyed with your `whsec_` secret.

use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::{Error, Result};
use crate::types::WebhookEvent;

/// Default tolerance for the signature timestamp, in seconds.
pub const DEFAULT_TOLERANCE_SECS: u64 = 300;

fn mac(secret: &str, t: i64, body: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(format!("{t}.").as_bytes());
    mac.update(body);
    mac
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Check the signature over the raw body (exactly as received) and return the
/// parsed event.
pub fn verify_webhook(raw_body: &[u8], signature_header: Option<&str>, secret: &str) -> Result<WebhookEvent> {
    verify_webhook_at(raw_body, signature_header, secret, DEFAULT_TOLERANCE_SECS, now_secs())
}

/// [`verify_webhook`] with an explicit tolerance and clock, for tests.
pub fn verify_webhook_at(
    raw_body: &[u8],
    signature_header: Option<&str>,
    secret: &str,
    tolerance_secs: u64,
    now: i64,
) -> Result<WebhookEvent> {
    let header = signature_header.ok_or(Error::Webhook("missing OpenIntents-Signature header"))?;
    let (mut t, mut v1) = (None, None);
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", v)) => t = v.parse::<i64>().ok(),
            Some(("v1", v)) => v1 = hex::decode(v).ok(),
            _ => {}
        }
    }
    let (t, v1) = t
        .zip(v1)
        .ok_or(Error::Webhook("malformed OpenIntents-Signature header"))?;
    if now.abs_diff(t) > tolerance_secs {
        return Err(Error::Webhook("signature timestamp outside tolerance"));
    }
    // verify_slice compares in constant time.
    mac(secret, t, raw_body)
        .verify_slice(&v1)
        .map_err(|_| Error::Webhook("signature mismatch"))?;
    Ok(serde_json::from_slice(raw_body)?)
}

/// Build a signature header, for tests and local webhook replays.
pub fn sign_webhook(raw_body: &[u8], secret: &str, t: Option<i64>) -> String {
    let t = t.unwrap_or_else(now_secs);
    format!(
        "t={t},v1={}",
        hex::encode(mac(secret, t, raw_body).finalize().into_bytes())
    )
}
