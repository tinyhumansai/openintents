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
        println!("send the payer: {}", intent.payment.unwrap().payment_url);
    }
    Ok(())
}
```

Also: `get_intent`, `list_intents`, `cancel_intent`, `create_customer`,
`get_customer`, `update_customer`, `list_customers`, and
`webhooks::verify_webhook` for the `OpenIntents-Signature` header (session
events carry a session: see `WebhookData`). `CreateIntent::customer` buys
for one of your customers. Unknown fields and statuses from newer API
versions are tolerated. `Error::Api` has the API's `code`, and the `lease`
with `profile_busy`.

## Live events and browser sessions

`stream_intent_events` follows an intent as it runs: an `EventStream`, read
with `next()` or as a futures `Stream`. It resumes by itself after a dropped
connection, and ends once the intent is over. `list_intent_events` pages
the same timeline.

A session opens a payer's browser profile without an intent: to sign them in
to a shop once, so later intents find them signed in. A session never pays
or places an order.

```rust
use openintents::{Client, CreateSession, SessionCommand};

async fn sign_in(oi: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let session = oi
        .create_session(CreateSession::default().customer("cus_4b81d0").start_url("https://shop.example/login"))
        .await?;
    let mut events = oi.stream_session_events(&session.id, None);
    while let Some(event) = events.next().await {
        let event = event?;
        if event.kind == "status" && event.data["status"] == "open" {
            break;
        }
    }
    let page = oi.run_session_command(&session.id, SessionCommand::snapshot()).await?;
    println!("{}", page.snapshot.unwrap_or_default()); // the refs to act on
    oi.run_session_command(&session.id, SessionCommand::type_text("e12", "ada@example.com"))
        .await?;
    let jpeg: Vec<u8> = oi.session_screenshot(&session.id).await?;
    println!("{} bytes of JPEG", jpeg.len());
    oi.release_session(&session.id).await?; // the profile keeps the sign-in
    Ok(())
}
```

Also `list_sessions`, `get_session`, `get_session_command`,
`list_session_events`, and for profiles `list_profiles`, `create_profile`,
`get_profile` and `forget_profile`.

A CLI binary is included behind a feature:

```bash
cargo install openintents --features cli
openintents login
openintents run "flat white, oat milk" --max 10 --wait
openintents customers create --external-id u-7 --name Ada --per-intent 50
openintents customers portal <id>          # a link to the customer's own page
openintents sessions open --customer <id> --start-url https://shop.example/
openintents sessions command <id> snapshot # or navigate, read, click, type, select, press
openintents sessions live <id>             # a 10 minute link to drive the browser
openintents profiles list --customer <id>
openintents sign-ins create https://www.amazon.com/ --customer <id>
echo '{"program":"Flying Blue","member_number":"1234567890"}' | openintents secrets set --kind loyalty --label "Flying Blue" --stdin
openintents secrets list                   # handles and masks, never values
```

`login` saves your key where the npm CLI does (`~/.config/openintents/config.json`)
and later commands read it; `OPENINTENTS_API_KEY` wins when set. The commands
and flags match the npm CLI's (`openintents --help`), except `mcp`, which only
the npm CLI has. Stored secrets are write-only: no command prints a value, and
`secrets set --stdin` takes the fields as a JSON object so they stay out of
the process list. Exit codes: `0` completed, `1` failed or error, `2` needs
payment, `3` cancelled, `4` needs the payer to sign in, `5` needs an answer.

## Country, waiting and forms

`.country()` or `.end_user_ip()` on `CreateIntent`, `CreateSession`,
`CreateSignIn` and `CreatePortalSession` picks the country the browser goes
out from; the object says which in `country` and `country_source`.
`create_intent_waiting(params, 30)` and `get_intent_waiting(id, 30)` have the
API hold the request up to 50 seconds until the intent finishes or pauses.

A question in `input_request` can offer `choices` (answer with one's `id`) or
ask a form of `fields` (answer with `AnswerInput::values(id, [("name", "value")])`);
`expects` says what a plain answer looks like. A `sensitive` question (an
identity document) is answered only on `input_request.url`. With
`.pay_at_shop(true)` on create (live keys), the payer pays the shop
themselves: cash on delivery where the shop offers it (the order goes through,
the receipt's `paid_with` is `cash_on_delivery`), else an `InputKind::Payment`
question shows the shop's UPI QR code at `input_request.image_url` (and the
same payment as a link in `input_request.upi_url`, when the page gave one),
answered with `AnswerInput::done(id)` once they paid (`paid_with` `upi_qr`).
A ride is paid to the driver in cash: its receipt says `kind` `ride` and
`state` `requested` (the app is looking for a driver; follow it, and find its
PIN, in the ride app). A cancel after the order was placed at the shop still
stops, but answers `error.code` `may_be_placed`: check with the shop.

## Secrets, sign-ins and live links

```rust
use openintents::{Client, CreateIntent, CreateSecret, CreateSignIn, SecretKind};

async fn buy(oi: &Client) -> openintents::Result<()> {
    let miles = oi
        .create_secret(
            CreateSecret::new(SecretKind::Loyalty, "Flying Blue", [("program", "Flying Blue"), ("member_number", "1234567890")])
                .shops(["airfrance.com"]),
        )
        .await?;
    oi.create_intent(CreateIntent::new("book the 7am flight").secrets([miles.id]).shops(["airfrance.com"]))
        .await?;
    let sign_in = oi.create_sign_in(CreateSignIn::new("https://www.amazon.com/")).await?; // send sign_in.url
    Ok(())
}
```

Secrets are write-only (`list_secrets`, `get_secret`, `replace_secret`,
`delete_secret`, `list_secret_uses`). Also `create_secret_request`,
`create_portal_session` and `create_live_link`.

## Webhook endpoints

`create_webhook_endpoint` (its `secret` is shown there only),
`list_webhook_endpoints`, `get_webhook_endpoint`, `update_webhook_endpoint`,
`delete_webhook_endpoint`, `rotate_webhook_secret` (the old secret signs for
24 more hours; `verify_webhook` accepts a header that carries both),
`test_webhook_endpoint`, `list_webhook_deliveries` and `redeliver_webhook`.
A 429 `Error::Api` carries `retry_after` and `rate_limit` (from the
`RateLimit-*` headers).

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --features cli -- -D warnings
cargo test
```

Docs: <https://openintents.io/docs>. License: GPL-3.0-or-later.
