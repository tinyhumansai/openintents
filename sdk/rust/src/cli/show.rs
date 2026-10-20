//! How the `openintents` command prints what the API returns. With `--json`
//! each object is one line of JSON; otherwise a few lines a person can read.

use openintents::{
    Customer, InputKind, Intent, LiveLink, PortalSession, Profile, Secret, SecretRequestCreated, Session,
    SessionCommandResult, SignIn,
};
use serde::Serialize;

/// The wire name of an enum value (`"in_use"`), for printing.
fn tag<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

/// `$12.50`, from cents.
pub fn money(cents: i64, currency: &str) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let n = cents.abs();
    let symbol = if currency.eq_ignore_ascii_case("usd") {
        "$".to_owned()
    } else {
        format!("{} ", currency.to_uppercase())
    };
    format!("{sign}{symbol}{}.{:02}", n / 100, n % 100)
}

pub fn json_line<T: Serialize>(value: &T) {
    println!("{}", serde_json::to_string(value).unwrap_or_default());
}

fn out<T: Serialize>(value: &T, json: bool, human: impl FnOnce() -> String) {
    if json {
        json_line(value);
    } else {
        println!("{}", human());
    }
}

pub fn intent(intent: &Intent, json: bool) {
    if json {
        return json_line(intent);
    }
    println!("{}  {}  \"{}\"", intent.id, intent.status.as_str(), intent.intent);
    if let Some(p) = &intent.payment {
        println!("  pay here: {}", p.payment_url);
    }
    if let Some(s) = &intent.sign_in {
        println!("  sign-in needed at {}: send this link to the payer only", s.shop);
        println!("  sign in here: {}", s.url);
    }
    if let Some(r) = &intent.input_request {
        let answer = if r.kind == InputKind::Approval {
            "--done"
        } else {
            "--value <answer>"
        };
        println!("  input needed at {}, ask the payer: {}", r.shop, r.prompt);
        if !r.options.is_empty() {
            println!("  options: {}", r.options.join(", "));
        }
        println!(
            "  answer: openintents input {} --input {} {answer} (or --decline)",
            intent.id, r.id
        );
        println!("  or the payer answers here (theirs only): {}", r.url);
    }
    if let Some(r) = &intent.result {
        println!("  {}: {} {} (fee {})", r.merchant, r.total, r.currency, r.fee);
    }
    if let Some(e) = &intent.error {
        println!("  error {}: {}", e.code, e.message);
    }
}

pub fn customer(c: &Customer, json: bool) {
    out(c, json, || {
        let who = [c.name.as_deref(), c.email.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        let mut text = format!(
            "{}  {}{}",
            c.id,
            c.external_id.as_deref().unwrap_or("-"),
            if who.is_empty() {
                String::new()
            } else {
                format!("  {who}")
            }
        );
        if let Some(l) = &c.limits {
            if l.per_intent.is_some() || l.monthly.is_some() {
                let cap = |v: Option<i64>| v.map_or("none".to_owned(), |c| money(c, "usd"));
                text += &format!(
                    "\n  limits: {} per intent, {} a month",
                    cap(l.per_intent),
                    cap(l.monthly)
                );
            }
        }
        text
    });
}

pub fn portal(p: &PortalSession, json: bool) {
    out(p, json, || {
        format!(
            "{}  portal for {}, goes out from {}\n  send this link to the customer only (works once, until {}): {}",
            p.id, p.customer, p.country, p.expires_at, p.url
        )
    });
}

pub fn session(s: &Session, json: bool) {
    out(s, json, || {
        let mut text = format!(
            "{}  {}  profile {}{}",
            s.id,
            s.status.as_str(),
            s.profile,
            s.url.as_ref().map_or(String::new(), |u| format!("  at {u}"))
        );
        if let Some(reason) = &s.close_reason {
            text += &format!("\n  closed: {reason}");
        }
        text
    });
}

pub fn live_link(l: &LiveLink, json: bool) {
    out(l, json, || {
        format!(
            "{}  live view\n  whoever holds this link can drive the session (until {}): {}",
            l.session, l.expires_at, l.url
        )
    });
}

/// A session command's result. Returns the exit code: 1 when it was refused or failed.
pub fn command(r: &SessionCommandResult, json: bool) -> u8 {
    out(r, json, || {
        let mut text = format!(
            "{}  {}  {}{}",
            r.id,
            r.kind,
            tag(&r.status),
            r.url.as_ref().map_or(String::new(), |u| format!("  at {u}"))
        );
        if let Some(e) = &r.error {
            text += &format!("\n  {}: {}", e.code, e.message);
        }
        for body in [&r.snapshot, &r.text].into_iter().flatten() {
            text += &format!("\n{body}");
        }
        text
    });
    match tag(&r.status).as_str() {
        "refused" | "failed" => 1,
        _ => 0,
    }
}

pub fn profile(p: &Profile, json: bool) {
    out(p, json, || {
        format!(
            "{}  {}  {}",
            p.id,
            tag(&p.status),
            p.customer
                .as_ref()
                .map_or("your account".to_owned(), |c| format!("customer {c}"))
        )
    });
}

pub fn sign_in(s: &SignIn, json: bool) {
    out(s, json, || {
        let mut text = format!("{}  {}  {}", s.id, tag(&s.status), s.shop);
        if let Some(url) = &s.url {
            text += &format!("\n  send this link to the payer only: {url}");
        }
        text
    });
}

/// A secret as a person may see it: its handle, kind, name and mask. Never a value.
pub fn secret(s: &Secret, json: bool) {
    out(s, json, || {
        let mut text = format!(
            "{}  {}  {}  {}{}",
            s.id,
            tag(&s.kind),
            s.label,
            s.mask,
            if s.default { "  (default)" } else { "" }
        );
        if !s.shops.is_empty() {
            text += &format!("\n  shops: {}", s.shops.join(", "));
        }
        text
    });
}

pub fn secret_request(r: &SecretRequestCreated, json: bool) {
    out(r, json, || {
        let kinds: Vec<String> = r.kinds.iter().map(tag).collect();
        format!(
            "{}  asks {} for {}\n  send this link to the customer only (until {}): {}",
            r.id,
            r.customer,
            kinds.join(", "),
            r.expires_at,
            r.url
        )
    });
}
