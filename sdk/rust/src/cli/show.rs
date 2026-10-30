//! How the `openintents` command prints what the API returns. With `--json`
//! each object is one line of JSON; otherwise a few lines a person can read.

use openintents::{
    Customer, InputKind, Intent, LiveLink, OrderPaymentMethod, OrderReview, PortalSession, Profile, Secret,
    SecretRequestCreated, Session, SessionCommandResult, SignIn,
};
use serde::Serialize;

/// How `openintents input` answers a question of `kind`: `--done` for one that
/// takes no value (an approval, a payment, a check), else a value.
fn answer_flag(kind: &InputKind) -> &'static str {
    match kind {
        InputKind::Approval | InputKind::Payment | InputKind::Check => "--done",
        _ => "--value <answer>",
    }
}

/// How `openintents input` answers a question of `kind`, with the other ways.
fn answer_hint(kind: &InputKind) -> String {
    match kind {
        InputKind::Review => "--done to place it (or --value <change>, or --decline)".to_owned(),
        _ => format!("{} (or --decline)", answer_flag(kind)),
    }
}

/// The lines that show an order to review.
fn review_lines(o: &OrderReview) -> Vec<String> {
    let mut lines = vec![format!("  order to review at {}:", o.merchant)];
    for item in &o.items {
        lines.push(format!(
            "    {} x {}  {}",
            item.quantity,
            item.name,
            money(item.amount, &o.currency)
        ));
    }
    for fee in &o.fees {
        lines.push(format!("    {}  {}", fee.label, money(fee.amount, &o.currency)));
    }
    lines.push(format!("    total  {}", money(o.total, &o.currency)));
    if let Some(to) = &o.deliver_to {
        lines.push(format!("    deliver to: {to}"));
    }
    if let Some(eta) = &o.eta {
        lines.push(format!("    arrives: {eta}"));
    }
    let method = match o.payment_method {
        OrderPaymentMethod::CashOnDelivery => "cash on delivery",
        OrderPaymentMethod::UpiQr => "UPI QR code",
        OrderPaymentMethod::Wallet => "wallet",
        OrderPaymentMethod::PaymentLink => "payment link",
        OrderPaymentMethod::Unknown => "unknown",
    };
    lines.push(format!("    pay by: {method}"));
    lines
}

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
        let answer = answer_hint(&r.kind);
        println!("  input needed at {}, ask the payer: {}", r.shop, r.prompt);
        if let Some(review) = &r.review {
            for line in review_lines(review) {
                println!("{line}");
            }
        }
        if let Some(image) = &r.image_url {
            println!("  payment code to scan (theirs only): {image}");
        }
        if let Some(link) = &r.upi_url {
            println!("  or open in a UPI app on the phone: {link}");
        }
        if !r.options.is_empty() {
            println!("  options: {}", r.options.join(", "));
        }
        println!("  answer: openintents input {} --input {} {answer}", intent.id, r.id);
        println!("  or the payer answers here (theirs only): {}", r.url);
    }
    if let Some(r) = &intent.result {
        println!("  {}: {} {} (fee {})", r.merchant, r.total, r.currency, r.fee);
        if let (Some(kind), Some(state)) = (&r.kind, &r.state) {
            println!("  {kind} {state}");
        }
        if let Some(to) = &r.deliver_to {
            println!("  delivered to: {to}");
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_that_takes_no_value_is_answered_done_a_check_among_them() {
        for kind in [InputKind::Approval, InputKind::Payment, InputKind::Check] {
            assert_eq!(answer_flag(&kind), "--done");
        }
        assert_eq!(answer_flag(&InputKind::Code), "--value <answer>");
    }

    #[test]
    fn a_review_is_answered_done_a_change_or_a_decline() {
        assert_eq!(
            answer_hint(&InputKind::Review),
            "--done to place it (or --value <change>, or --decline)"
        );
        assert_eq!(answer_hint(&InputKind::Check), "--done (or --decline)");
    }

    #[test]
    fn a_review_shows_items_fees_total_address_and_way_to_pay() {
        let review: OrderReview = serde_json::from_value(serde_json::json!({
            "merchant": "blinkit.com", "currency": "inr", "total": 11500,
            "items": [{ "name": "Protein bar", "quantity": 2, "amount": 10000 }],
            "fees": [{ "label": "Delivery fee", "amount": 1500 }],
            "deliver_to": "Home", "eta": null, "payment_method": "upi_qr"
        }))
        .unwrap();
        assert_eq!(
            review_lines(&review),
            [
                "  order to review at blinkit.com:",
                "    2 x Protein bar  INR 100.00",
                "    Delivery fee  INR 15.00",
                "    total  INR 115.00",
                "    deliver to: Home",
                "    pay by: UPI QR code",
            ]
        );
    }
}
