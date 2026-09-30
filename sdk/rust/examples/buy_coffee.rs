//! cargo run --example buy_coffee   (needs OPENINTENTS_API_KEY; use an oi_test_ key)

use openintents::{Client, CreateIntent, IntentStatus};

// Returning the SDK error from main keeps the example short; it is large on
// purpose (it carries the API error fields).
#[allow(clippy::result_large_err)]
#[tokio::main]
async fn main() -> openintents::Result<()> {
    let oi = Client::from_env()?;
    let created = oi
        .create_intent(
            CreateIntent::new("flat white, oat milk, pickup 8:45 at the Blue Bottle on 5th").max_amount(1000),
        )
        .await?;
    println!("created {}", created.id);

    let intent = oi.wait(&created.id, Default::default()).await?;
    match intent.status {
        IntentStatus::RequiresPayment => println!("Pay here: {}", intent.payment.unwrap().payment_url),
        IntentStatus::Completed => println!("Receipt: {:?}", intent.result),
        other => println!("Ended as {}: {:?}", other.as_str(), intent.error),
    }
    Ok(())
}
