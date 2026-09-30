# openintents (Rust)

Rust SDK for [OpenIntents](https://openintents.io), the agentic API to buy
anything. Async (tokio + reqwest with rustls).

> Preview: the API is still being built, so it may change before launch.

```toml
[dependencies]
openintents = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use openintents::{Client, CreateIntent, IntentStatus};

#[tokio::main]
async fn main() -> openintents::Result<()> {
    let oi = Client::from_env()?; // OPENINTENTS_API_KEY
    let intent = oi
        .create_intent(CreateIntent::new("book an Uber to SFO at 7am").max_amount(6000))
        .await?;
    let intent = oi.wait(&intent.id, Default::default()).await?;
    if intent.status == IntentStatus::RequiresPayment {
        println!("pay here: {}", intent.payment.unwrap().payment_url);
    }
    Ok(())
}
```

Also: `get_intent`, `list_intents`, `cancel_intent`, `balance`, and
`webhooks::verify_webhook` for the `OpenIntents-Signature` header. Unknown
fields and statuses from newer API versions are tolerated.

A CLI binary is included behind a feature:

```bash
cargo install openintents --features cli
openintents run "flat white, oat milk" --max 10 --wait
```

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --features cli -- -D warnings
cargo test
```

Docs: <https://openintents.io/docs>. License: GPL-3.0-or-later.
