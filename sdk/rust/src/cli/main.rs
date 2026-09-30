//! `openintents` CLI (build with `--features cli`). Same commands and exit
//! codes as the npm CLI: 0 completed, 1 failed or error, 2 needs payment,
//! 3 cancelled.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use openintents::{Client, CreateIntent, Intent, IntentStatus, ListIntents};

#[derive(Parser)]
#[command(name = "openintents", version, about = "The agentic API to buy anything")]
struct Cli {
    /// Print one JSON object per line.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create an intent.
    Run {
        intent: String,
        /// Spending cap in dollars.
        #[arg(long)]
        max: Option<f64>,
        /// Always return a payment link instead of paying from the balance.
        #[arg(long)]
        payment_link: bool,
        /// Wait until it completes, fails or needs payment.
        #[arg(long)]
        wait: bool,
    },
    /// Show an intent.
    Status {
        id: String,
        #[arg(long)]
        wait: bool,
    },
    /// List recent intents.
    List {
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Cancel an intent that hasn't paid.
    Cancel { id: String },
    /// Print an intent's payment link.
    Pay { id: String },
    /// Show the agent balance.
    Balance,
}

fn exit_for(status: IntentStatus) -> u8 {
    match status {
        IntentStatus::Failed | IntentStatus::Unknown => 1,
        IntentStatus::RequiresPayment => 2,
        IntentStatus::Cancelled => 3,
        _ => 0,
    }
}

fn show(intent: &Intent, json: bool) {
    if json {
        println!("{}", serde_json::to_string(intent).unwrap_or_default());
        return;
    }
    println!("{}  {}  \"{}\"", intent.id, intent.status.as_str(), intent.intent);
    if let Some(p) = &intent.payment {
        println!("  pay here: {}", p.payment_url);
    }
    if let Some(r) = &intent.result {
        println!("  {}: {} {} (fee {})", r.merchant, r.total, r.currency, r.fee);
    }
    if let Some(e) = &intent.error {
        println!("  error {}: {}", e.code, e.message);
    }
}

async fn run(cli: Cli) -> openintents::Result<u8> {
    let oi = Client::from_env()?;
    let json = cli.json;
    Ok(match cli.command {
        Command::Run {
            intent,
            max,
            payment_link,
            wait,
        } => {
            let mut params = CreateIntent::new(intent);
            if let Some(dollars) = max {
                params = params.max_amount((dollars * 100.0).round() as i64);
            }
            if payment_link {
                params = params.require_payment_link(true);
            }
            let mut it = oi.create_intent(params).await?;
            if wait {
                it = oi.wait(&it.id, Default::default()).await?;
            }
            show(&it, json);
            exit_for(it.status)
        }
        Command::Status { id, wait } => {
            let it = if wait {
                oi.wait(&id, Default::default()).await?
            } else {
                oi.get_intent(&id).await?
            };
            show(&it, json);
            exit_for(it.status)
        }
        Command::List { limit } => {
            let page = oi
                .list_intents(&ListIntents {
                    limit,
                    ..Default::default()
                })
                .await?;
            for it in &page.data {
                show(it, json);
            }
            0
        }
        Command::Cancel { id } => {
            let it = oi.cancel_intent(&id).await?;
            show(&it, json);
            exit_for(it.status)
        }
        Command::Pay { id } => {
            let it = oi.get_intent(&id).await?;
            match &it.payment {
                Some(p) => println!("{}", p.payment_url),
                None => println!("{} doesn't need payment ({})", it.id, it.status.as_str()),
            }
            exit_for(it.status)
        }
        Command::Balance => {
            let b = oi.balance().await?;
            if json {
                println!("{}", serde_json::to_string(&b).unwrap_or_default());
            } else {
                println!("{} {} available", b.available, b.currency);
            }
            0
        }
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("openintents: {err}");
            ExitCode::from(1)
        }
    }
}
