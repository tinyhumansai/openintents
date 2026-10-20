//! `openintents` CLI (build with `--features cli`). Same commands and exit
//! codes as the npm CLI: 0 completed, 1 failed or error, 2 needs payment,
//! 3 cancelled, 4 needs the payer to sign in, 5 needs an answer from the
//! payer. (`mcp`, the stdio bridge, is npm-only.)

mod show;

use std::collections::HashMap;
use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::builder::PossibleValuesParser;
use clap::{Args, Parser, Subcommand};
use openintents::{
    AnswerInput, Client, CreateIntent, CreatePortalSession, CreateProfile, CreateSecret, CreateSecretRequest,
    CreateSession, CreateSignIn, CreateTopup, CustomerParams, IntentStatus, LimitsParams, ListCustomers, ListIntents,
    ListProfiles, ListSecrets, ListSessions, SecretKind, SessionCommand, SessionStatus,
};
use serde_json::{json, Value};

const DASHBOARD_KEYS_URL: &str = "https://openintents.io/dashboard/api-keys";

const INTENT_STATUSES: [&str; 8] = [
    "queued",
    "running",
    "requires_payment",
    "requires_input",
    "requires_login",
    "completed",
    "failed",
    "cancelled",
];
const SESSION_STATUSES: [&str; 6] = ["queued", "opening", "open", "closing", "closed", "failed"];
const SECRET_KINDS: [&str; 6] = [
    "card",
    "address",
    "contact",
    "identity_document",
    "shop_password",
    "loyalty",
];

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
        /// Context for the agent, as key=value; repeat for more.
        #[arg(long)]
        context: Vec<String>,
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
        #[arg(long, value_parser = PossibleValuesParser::new(INTENT_STATUSES))]
        status: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Cancel an intent that hasn't paid.
    Cancel { id: String },
    /// Answer what an intent asks the payer for.
    Input {
        id: String,
        /// The ID of the intent's input request.
        #[arg(long)]
        input: String,
        #[command(flatten)]
        answer: Answer,
    },
    /// Print an intent's payment link.
    Pay { id: String },
    /// Show the agent balance.
    Balance {
        /// A customer's balance instead of your account's.
        #[arg(long)]
        customer: Option<String>,
    },
    /// Create, update, list customers and make a link to their own page.
    Customers {
        #[command(subcommand)]
        command: Customers,
    },
    /// Open a browser on a profile and drive it.
    Sessions {
        #[command(subcommand)]
        command: Sessions,
    },
    /// List, create and forget browser profiles.
    Profiles {
        #[command(subcommand)]
        command: Profiles,
    },
    /// Make a payment link that adds to a balance.
    Topups {
        #[command(subcommand)]
        command: Topups,
    },
    /// Make a link where a payer signs in to a shop.
    SignIns {
        #[command(subcommand)]
        command: SignIns,
    },
    /// Store details a purchase may need. Write-only: no command shows a value.
    Secrets {
        #[command(subcommand)]
        command: Secrets,
    },
    /// Save an API key for later commands.
    Login {
        /// Print the dashboard address instead of opening it.
        #[arg(long)]
        no_open: bool,
    },
}

/// Exactly one answer to an input request.
#[derive(Args)]
#[group(required = true, multiple = false)]
struct Answer {
    /// The phone number, email, code or a detail's answer.
    #[arg(long)]
    value: Option<String>,
    /// The payer would rather not.
    #[arg(long)]
    decline: bool,
    /// The payer approved the sign-in (for an approval).
    #[arg(long)]
    done: bool,
}

#[derive(Args)]
struct CustomerFlags {
    /// Your own ID for them, unique in your account.
    #[arg(long)]
    external_id: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    email: Option<String>,
    /// key=value; repeat for more. On update it replaces the whole map.
    #[arg(long)]
    metadata: Vec<String>,
    /// Most one intent may cost, in dollars, or `none`.
    #[arg(long)]
    per_intent: Option<String>,
    /// Most a month may cost, in dollars, or `none`.
    #[arg(long)]
    monthly: Option<String>,
}

#[derive(Subcommand)]
enum Customers {
    Create(CustomerFlags),
    Update {
        id: String,
        #[command(flatten)]
        flags: CustomerFlags,
    },
    Get {
        id: String,
    },
    List {
        #[arg(long)]
        external_id: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        cursor: Option<String>,
    },
    /// A link to the customer's own page: send it to them only.
    Portal {
        id: String,
        #[arg(long)]
        return_url: Option<String>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        end_user_ip: Option<String>,
        /// How long their session lasts once they open it: 60 to 86400.
        #[arg(long)]
        ttl: Option<u32>,
    },
}

#[derive(Subcommand)]
enum Sessions {
    /// Open a browser: on your account's profile, or --profile, or --customer's.
    Open {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        customer: Option<String>,
        #[arg(long)]
        start_url: Option<String>,
        /// 1 to 15.
        #[arg(long)]
        max_minutes: Option<u32>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        end_user_ip: Option<String>,
        #[arg(long)]
        webhook_url: Option<String>,
    },
    List {
        #[arg(long, value_parser = PossibleValuesParser::new(SESSION_STATUSES))]
        status: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        customer: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        cursor: Option<String>,
    },
    Get {
        id: String,
    },
    /// Close the browser, keeping what the session changed in the profile.
    Release {
        id: String,
    },
    /// A 10 minute link to drive the session: whoever holds it can.
    Live {
        id: String,
    },
    /// Run one command on the session's page. A session never pays.
    Command {
        id: String,
        #[command(subcommand)]
        action: Action,
    },
    /// Save a JPEG of the session's page.
    Screenshot {
        id: String,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum Action {
    Navigate { url: String },
    Snapshot,
    Read { selector: Option<String> },
    Click { element: String },
    Type { element: String, text: String },
    Select { element: String, value: String },
    Press { element: String, key: String },
}

#[derive(Subcommand)]
enum Profiles {
    List {
        #[arg(long)]
        customer: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        cursor: Option<String>,
    },
    Create {
        #[arg(long)]
        customer: Option<String>,
    },
    Get {
        id: String,
    },
    /// Erase everything the profile kept.
    Forget {
        id: String,
    },
}

#[derive(Subcommand)]
enum Topups {
    /// A payment link for an amount in dollars. Live keys only.
    Create {
        dollars: String,
        #[arg(long)]
        customer: Option<String>,
    },
    Get {
        id: String,
    },
}

#[derive(Subcommand)]
enum SignIns {
    Create {
        /// A public https address on the shop.
        shop_url: String,
        #[arg(long)]
        customer: Option<String>,
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        end_user_ip: Option<String>,
        #[arg(long)]
        return_url: Option<String>,
    },
    Get {
        id: String,
    },
}

#[derive(Subcommand)]
enum Secrets {
    /// Store a secret. Give its fields with --field name=value, or as a JSON
    /// object on stdin with --stdin to keep them out of the process list.
    Set {
        #[arg(long, value_parser = PossibleValuesParser::new(SECRET_KINDS))]
        kind: String,
        /// A name for it, up to 60 characters. Put no value in it.
        #[arg(long)]
        label: String,
        #[arg(long)]
        field: Vec<String>,
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        customer: Option<String>,
        /// A shop (domain) it may be used on; repeat for more.
        #[arg(long)]
        shop: Vec<String>,
        /// Offer this one first among the payer's secrets of its kind.
        #[arg(long)]
        default: bool,
        #[arg(long)]
        expires_at: Option<String>,
    },
    List {
        #[arg(long)]
        customer: Option<String>,
        #[arg(long, value_parser = PossibleValuesParser::new(SECRET_KINDS))]
        kind: Option<String>,
    },
    Get {
        id: String,
    },
    Delete {
        id: String,
    },
    /// Ask a customer to enter their own: send them the link.
    Request {
        customer: String,
        #[arg(long, required = true, value_parser = PossibleValuesParser::new(SECRET_KINDS))]
        kind: Vec<String>,
        #[arg(long)]
        return_url: Option<String>,
        #[arg(long)]
        ttl: Option<u32>,
    },
}

/// What went wrong, as one line. Never quotes a field's value.
struct Failure(String);

impl From<openintents::Error> for Failure {
    fn from(e: openintents::Error) -> Self {
        Failure(e.to_string())
    }
}

impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Self {
        Failure(e.to_string())
    }
}

type Outcome = Result<u8, Failure>;

fn fail<T>(message: impl Into<String>) -> Result<T, Failure> {
    Err(Failure(message.into()))
}

fn exit_for(status: IntentStatus) -> u8 {
    match status {
        IntentStatus::Failed | IntentStatus::Unknown => 1,
        IntentStatus::RequiresPayment => 2,
        IntentStatus::Cancelled => 3,
        IntentStatus::RequiresLogin => 4,
        IntentStatus::RequiresInput => 5,
        _ => 0,
    }
}

/// Dollars to cents, as the API counts: `"12.5"` is 1250.
fn cents(flag: &str, text: &str) -> Result<i64, Failure> {
    match text.trim().parse::<f64>() {
        Ok(d) if d.is_finite() && d > 0.0 => Ok((d * 100.0).round() as i64),
        _ => fail(format!("{flag} expects an amount in dollars, got \"{text}\"")),
    }
}

fn limit(flag: &str, text: &str) -> Result<Option<i64>, Failure> {
    if text == "none" {
        Ok(None)
    } else {
        cents(flag, text).map(Some)
    }
}

/// `key=value` pairs. Errors name the flag only, never the value.
fn pairs(flag: &str, items: &[String]) -> Result<HashMap<String, String>, Failure> {
    items
        .iter()
        .map(|kv| match kv.split_once('=') {
            Some((k, v)) if !k.is_empty() => Ok((k.to_owned(), v.to_owned())),
            _ => fail(format!("{flag} expects name=value")),
        })
        .collect()
}

fn customer_params(flags: CustomerFlags) -> Result<CustomerParams, Failure> {
    let metadata = pairs("--metadata", &flags.metadata)?;
    let limits = if flags.per_intent.is_some() || flags.monthly.is_some() {
        let mut limits = LimitsParams::default();
        if let Some(t) = &flags.per_intent {
            limits = limits.per_intent(limit("--per-intent", t)?);
        }
        if let Some(t) = &flags.monthly {
            limits = limits.monthly(limit("--monthly", t)?);
        }
        Some(limits)
    } else {
        None
    };
    Ok(CustomerParams {
        external_id: flags.external_id,
        name: flags.name,
        email: flags.email,
        metadata: (!metadata.is_empty()).then_some(metadata),
        limits,
    })
}

fn enum_of<T: serde::de::DeserializeOwned>(wire: &str) -> T {
    serde_json::from_value(Value::String(wire.to_owned())).expect("a value the flag already checked")
}

fn session_command(action: Action) -> SessionCommand {
    match action {
        Action::Navigate { url } => SessionCommand::navigate(url),
        Action::Snapshot => SessionCommand::snapshot(),
        Action::Read { selector: None } => SessionCommand::read(),
        Action::Read { selector: Some(s) } => SessionCommand::read_selector(s),
        Action::Click { element } => SessionCommand::click(element),
        Action::Type { element, text } => SessionCommand::type_text(element, text),
        Action::Select { element, value } => SessionCommand::select(element, value),
        Action::Press { element, key } => SessionCommand::press(element, key),
    }
}

/// The fields of a secret as a JSON object of strings. Errors never quote the input.
fn stdin_fields(text: &str) -> Result<HashMap<String, String>, Failure> {
    serde_json::from_str(text).or_else(|_| fail("--stdin expects a JSON object of field names to text values."))
}

/// One line on a terminal; everything piped in otherwise.
fn read_stdin() -> std::io::Result<String> {
    let mut text = String::new();
    if std::io::stdin().is_terminal() {
        std::io::stdin().read_line(&mut text)?;
    } else {
        std::io::stdin().read_to_string(&mut text)?;
    }
    Ok(text)
}

fn config_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("openintents").join("config.json")
}

/// The key `openintents login` saved, if any.
fn saved_key() -> Option<String> {
    let text = std::fs::read_to_string(config_file()).ok()?;
    serde_json::from_str::<Value>(&text)
        .ok()?
        .get("apiKey")?
        .as_str()
        .map(str::to_owned)
}

/// The client: the key in OPENINTENTS_API_KEY, else the saved one.
fn client() -> Result<Client, Failure> {
    if let Ok(client) = Client::from_env() {
        return Ok(client);
    }
    let Some(key) = saved_key() else {
        return fail("No API key. Run `openintents login` or set OPENINTENTS_API_KEY.");
    };
    let mut builder = Client::builder(key);
    if let Ok(url) = std::env::var("OPENINTENTS_BASE_URL") {
        builder = builder.base_url(url);
    }
    Ok(builder.build())
}

fn open_in_browser(url: &str) {
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/c", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    };
    let _ = std::process::Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn login(no_open: bool) -> Outcome {
    println!(
        "Create a key at {DASHBOARD_KEYS_URL}{}.",
        if no_open { "" } else { " (opening it now)" }
    );
    if !no_open {
        open_in_browser(DASHBOARD_KEYS_URL);
    }
    println!("Paste your API key:");
    std::io::stdout().flush()?;
    let key = read_stdin()?.trim().to_owned();
    if !(key.starts_with("oi_live_") || key.starts_with("oi_test_")) {
        return fail("That doesn't look like an OpenIntents key (oi_live_... or oi_test_...).");
    }
    let file = config_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(&file)?;
    f.write_all(
        serde_json::to_string_pretty(&json!({ "apiKey": key }))
            .unwrap_or_default()
            .as_bytes(),
    )?;
    println!("Saved to {}.", file.display());
    Ok(0)
}

async fn run(cli: Cli) -> Outcome {
    let json = cli.json;
    if let Command::Login { no_open } = cli.command {
        return login(no_open);
    }
    let oi = client()?;
    Ok(match cli.command {
        Command::Login { .. } => unreachable!("handled above"),
        Command::Run {
            intent,
            max,
            context,
            payment_link,
            wait,
        } => {
            let mut params = CreateIntent::new(intent);
            if let Some(dollars) = max {
                params = params.max_amount((dollars * 100.0).round() as i64);
            }
            let context = pairs("--context", &context)?;
            if !context.is_empty() {
                params = params.context(json!(context));
            }
            if payment_link {
                params = params.require_payment_link(true);
            }
            let mut it = oi.create_intent(params).await?;
            if wait {
                it = oi.wait(&it.id, Default::default()).await?;
            }
            show::intent(&it, json);
            exit_for(it.status)
        }
        Command::Status { id, wait } => {
            let it = if wait {
                oi.wait(&id, Default::default()).await?
            } else {
                oi.get_intent(&id).await?
            };
            show::intent(&it, json);
            exit_for(it.status)
        }
        Command::List { status, limit } => {
            let page = oi
                .list_intents(&ListIntents {
                    status: status.as_deref().map(enum_of),
                    limit,
                    ..Default::default()
                })
                .await?;
            for it in &page.data {
                show::intent(it, json);
            }
            0
        }
        Command::Cancel { id } => {
            let it = oi.cancel_intent(&id).await?;
            show::intent(&it, json);
            exit_for(it.status)
        }
        Command::Input { id, input, answer } => {
            let answer = match answer {
                Answer { value: Some(value), .. } => AnswerInput::value(input, value),
                Answer { decline: true, .. } => AnswerInput::decline(input),
                _ => AnswerInput::done(input),
            };
            let it = oi.answer_intent_input(&id, answer).await?;
            show::intent(&it, json);
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
        Command::Balance { customer } => {
            let b = match customer {
                Some(c) => oi.balance_for(&c).await?,
                None => oi.balance().await?,
            };
            if json {
                show::json_line(&b);
            } else {
                println!("{} available", show::money(b.available, &b.currency));
            }
            0
        }
        Command::Customers { command } => {
            match command {
                Customers::Create(flags) => show::customer(&oi.create_customer(customer_params(flags)?).await?, json),
                Customers::Update { id, flags } => {
                    show::customer(&oi.update_customer(&id, customer_params(flags)?).await?, json)
                }
                Customers::Get { id } => show::customer(&oi.get_customer(&id).await?, json),
                Customers::List {
                    external_id,
                    limit,
                    cursor,
                } => {
                    let list = oi
                        .list_customers(&ListCustomers {
                            external_id,
                            limit,
                            cursor,
                        })
                        .await?;
                    for c in &list.data {
                        show::customer(c, json);
                    }
                }
                Customers::Portal {
                    id,
                    return_url,
                    country,
                    end_user_ip,
                    ttl,
                } => {
                    let params = CreatePortalSession {
                        return_url,
                        country,
                        end_user_ip,
                        ttl_seconds: ttl,
                    };
                    show::portal(&oi.create_portal_session(&id, params).await?, json);
                }
            }
            0
        }
        Command::Sessions { command } => match command {
            Sessions::Open {
                profile,
                customer,
                start_url,
                max_minutes,
                country,
                end_user_ip,
                webhook_url,
            } => {
                let params = CreateSession {
                    profile,
                    customer,
                    start_url,
                    max_minutes,
                    country,
                    end_user_ip,
                    webhook_url,
                };
                show::session(&oi.create_session(params).await?, json);
                0
            }
            Sessions::List {
                status,
                profile,
                customer,
                limit,
                cursor,
            } => {
                let list = oi
                    .list_sessions(&ListSessions {
                        status: status.as_deref().map(enum_of::<SessionStatus>),
                        profile,
                        customer,
                        limit,
                        cursor,
                    })
                    .await?;
                for s in &list.data {
                    show::session(s, json);
                }
                0
            }
            Sessions::Get { id } => {
                show::session(&oi.get_session(&id).await?, json);
                0
            }
            Sessions::Release { id } => {
                show::session(&oi.release_session(&id).await?, json);
                0
            }
            Sessions::Live { id } => {
                show::live_link(&oi.create_live_link(&id).await?, json);
                0
            }
            Sessions::Command { id, action } => {
                let result = oi.run_session_command(&id, session_command(action)).await?;
                show::command(&result, json)
            }
            Sessions::Screenshot { id, out } => {
                let jpeg = oi.session_screenshot(&id).await?;
                std::fs::write(&out, &jpeg)?;
                if json {
                    show::json_line(&json!({ "session": id, "file": out, "bytes": jpeg.len() }));
                } else {
                    println!("Saved {} bytes to {}", jpeg.len(), out.display());
                }
                0
            }
        },
        Command::Profiles { command } => {
            match command {
                Profiles::List {
                    customer,
                    limit,
                    cursor,
                } => {
                    let list = oi
                        .list_profiles(&ListProfiles {
                            customer,
                            limit,
                            cursor,
                        })
                        .await?;
                    for p in &list.data {
                        show::profile(p, json);
                    }
                }
                Profiles::Create { customer } => {
                    show::profile(&oi.create_profile(CreateProfile { customer }).await?, json);
                }
                Profiles::Get { id } => show::profile(&oi.get_profile(&id).await?, json),
                Profiles::Forget { id } => show::profile(&oi.forget_profile(&id).await?, json),
            }
            0
        }
        Command::Topups { command } => {
            match command {
                Topups::Create { dollars, customer } => {
                    let mut params = CreateTopup::new(cents("amount", &dollars)?);
                    params.customer = customer;
                    show::topup(&oi.create_topup(params).await?, json);
                }
                Topups::Get { id } => show::topup(&oi.get_topup(&id).await?, json),
            }
            0
        }
        Command::SignIns { command } => {
            match command {
                SignIns::Create {
                    shop_url,
                    customer,
                    country,
                    end_user_ip,
                    return_url,
                } => {
                    let mut params = CreateSignIn::new(shop_url);
                    params.customer = customer;
                    params.country = country;
                    params.end_user_ip = end_user_ip;
                    params.return_url = return_url;
                    show::sign_in(&oi.create_sign_in(params).await?, json);
                }
                SignIns::Get { id } => show::sign_in(&oi.get_sign_in(&id).await?, json),
            }
            0
        }
        Command::Secrets { command } => {
            match command {
                Secrets::Set {
                    kind,
                    label,
                    field,
                    stdin,
                    customer,
                    shop,
                    default,
                    expires_at,
                } => {
                    let mut fields = if stdin {
                        stdin_fields(&read_stdin()?)?
                    } else {
                        HashMap::new()
                    };
                    fields.extend(pairs("--field", &field)?);
                    if fields.is_empty() {
                        return fail("A secret needs fields: pass --field name=value or --stdin.");
                    }
                    let mut params = CreateSecret::new(enum_of::<SecretKind>(&kind), label, fields);
                    if let Some(c) = customer {
                        params = params.customer(c);
                    }
                    if !shop.is_empty() {
                        params = params.shops(shop);
                    }
                    if default {
                        params = params.default(true);
                    }
                    if let Some(at) = expires_at {
                        params = params.expires_at(at);
                    }
                    show::secret(&oi.create_secret(params).await?, json);
                }
                Secrets::List { customer, kind } => {
                    let list = oi
                        .list_secrets(&ListSecrets {
                            customer,
                            kind: kind.as_deref().map(enum_of),
                        })
                        .await?;
                    for s in &list.data {
                        show::secret(s, json);
                    }
                }
                Secrets::Get { id } => show::secret(&oi.get_secret(&id).await?, json),
                Secrets::Delete { id } => {
                    oi.delete_secret(&id).await?;
                    if json {
                        show::json_line(&json!({ "id": id, "deleted": true }));
                    } else {
                        println!("Deleted {id}");
                    }
                }
                Secrets::Request {
                    customer,
                    kind,
                    return_url,
                    ttl,
                } => {
                    let mut params = CreateSecretRequest::new(kind.iter().map(|k| enum_of::<SecretKind>(k)));
                    if let Some(u) = return_url {
                        params = params.return_url(u);
                    }
                    if let Some(t) = ttl {
                        params = params.ttl_seconds(t);
                    }
                    show::secret_request(&oi.create_secret_request(&customer, params).await?, json);
                }
            }
            0
        }
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli).await {
        Ok(code) => ExitCode::from(code),
        Err(Failure(message)) => {
            if json {
                show::json_line(&json!({ "error": { "message": message } }));
            } else {
                eprintln!("openintents: {message}");
            }
            ExitCode::from(1)
        }
    }
}
