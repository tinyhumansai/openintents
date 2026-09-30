//! Rust SDK for [OpenIntents](https://openintents.io): let your agents buy
//! anything. Send an intent in plain language and it gets bought end to end in
//! a real browser, paid (from your agent balance, or a payment link) and
//! resolved to a receipt.
//!
//! ```no_run
//! # async fn run() -> openintents::Result<()> {
//! use openintents::{Client, CreateIntent, IntentStatus};
//!
//! let oi = Client::from_env()?;
//! let intent = oi
//!     .create_intent(CreateIntent::new("book an Uber to SFO at 7am").max_amount(6000))
//!     .await?;
//! let intent = oi.wait(&intent.id, Default::default()).await?;
//! match intent.status {
//!     IntentStatus::RequiresPayment => println!("pay: {}", intent.payment.unwrap().payment_url),
//!     IntentStatus::Completed => println!("done: {:?}", intent.result),
//!     other => println!("ended as {}", other.as_str()),
//! }
//! # Ok(()) }
//! ```
//!
//! Preview: the API is still being built and may change before launch.

mod client;
mod error;
pub mod types;
pub mod webhooks;

pub use client::{Client, ClientBuilder, WaitOptions, DEFAULT_BASE_URL};
pub use error::{Error, Result};
pub use types::*;
pub use webhooks::{sign_webhook, verify_webhook};
