//! The `openintents` binary, run against a scripted API: one test per command.
//! Build with `--features cli` (`cargo test --features cli`).

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Run {
    code: i32,
    out: String,
    err: String,
}

/// Runs the binary with `args` against `server`, a throwaway config dir and `stdin`.
async fn openintents(server: &MockServer, args: &[&str], stdin: &str) -> Run {
    let base = server.uri();
    let config = std::env::temp_dir().join(format!("oi-cli-{}", uuid_like()));
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let stdin = stdin.to_string();
    tokio::task::spawn_blocking(move || {
        let mut child = Command::new(env!("CARGO_BIN_EXE_openintents"))
            .args(&args)
            .env("OPENINTENTS_API_KEY", "oi_test_x")
            .env("OPENINTENTS_BASE_URL", base)
            .env("XDG_CONFIG_HOME", &config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn openintents");
        child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        Run {
            code: out.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    })
    .await
    .unwrap()
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("{n}-{}", std::process::id())
}

async fn serve(verb: &str, at: &str, status: u16, body: Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method(verb))
        .and(path(at))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

async fn sent(server: &MockServer) -> (String, Option<Value>) {
    let req = server.received_requests().await.unwrap().remove(0);
    let query = req.url.query().map(|q| format!("?{q}")).unwrap_or_default();
    (
        format!("{} {}{query}", req.method, req.url.path()),
        serde_json::from_slice(&req.body).ok(),
    )
}

fn customer() -> Value {
    json!({ "id": "cus_1", "object": "customer", "external_id": "u-7", "name": "Ada", "email": "ada@example.com",
            "metadata": {}, "created_at": "2026-10-01T00:00:00Z" })
}

fn session(status: &str) -> Value {
    json!({ "id": "ses_1", "object": "session", "profile": "prf_1", "status": status, "test": true,
            "created_at": "2026-10-01T00:00:00Z", "expires_at": "2026-10-01T00:15:00Z", "url": "https://shop.example/" })
}

fn profile(status: &str) -> Value {
    json!({ "id": "prf_1", "object": "profile", "customer": "cus_1", "status": status, "created_at": "2026-10-01T00:00:00Z" })
}

fn secret() -> Value {
    json!({ "id": "sec_1", "object": "secret", "customer": null, "kind": "card", "label": "Work Visa", "mask": "Visa ••4242",
            "public": {}, "shops": ["airfrance.com"], "default": false, "mode": "test", "entered_by": "developer", "version": 1,
            "created_at": "x", "updated_at": "x", "expires_at": null, "last_used_at": null })
}

fn command_result(status: &str) -> Value {
    json!({ "id": "cmd_1", "object": "session_command", "type": "x", "status": status, "created_at": "x" })
}

#[tokio::test]
async fn customers_create_sends_the_flags_with_limits_in_cents() {
    let server = serve("POST", "/customers", 201, customer()).await;
    let r = openintents(
        &server,
        &[
            "customers",
            "create",
            "--external-id",
            "u-7",
            "--name",
            "Ada",
            "--email",
            "ada@example.com",
            "--metadata",
            "plan=pro",
            "--per-intent",
            "50",
            "--monthly",
            "none",
        ],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    let (line, body) = sent(&server).await;
    assert_eq!(line, "POST /customers");
    assert_eq!(
        body.unwrap(),
        json!({ "external_id": "u-7", "name": "Ada", "email": "ada@example.com", "metadata": { "plan": "pro" },
                "limits": { "per_intent": 5000, "monthly": null } })
    );
    assert!(r.out.contains("cus_1") && r.out.contains("u-7"), "{}", r.out);
}

#[tokio::test]
async fn customers_get_update_list_and_portal() {
    let server = serve("GET", "/customers/cus_1", 200, customer()).await;
    assert_eq!(openintents(&server, &["customers", "get", "cus_1"], "").await.code, 0);

    let server = serve("PATCH", "/customers/cus_1", 200, customer()).await;
    let r = openintents(&server, &["customers", "update", "cus_1", "--name", "Ada L"], "").await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(sent(&server).await.1.unwrap(), json!({ "name": "Ada L" }));

    let server = serve(
        "GET",
        "/customers",
        200,
        json!({ "object": "list", "data": [customer()] }),
    )
    .await;
    let r = openintents(
        &server,
        &["customers", "list", "--external-id", "u-7", "--limit", "5"],
        "",
    )
    .await;
    assert_eq!(sent(&server).await.0, "GET /customers?external_id=u-7&limit=5");
    assert!(r.out.contains("cus_1"));

    let portal = json!({ "id": "por_1", "object": "portal_session", "customer": "cus_1", "country": "de",
                         "country_source": "explicit", "url": "https://openintents.io/portal/abc", "expires_at": "x" });
    let server = serve("POST", "/customers/cus_1/portal_sessions", 201, portal).await;
    let r = openintents(
        &server,
        &[
            "customers",
            "portal",
            "cus_1",
            "--return-url",
            "https://app.example/back",
            "--country",
            "de",
            "--ttl",
            "3600",
        ],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        sent(&server).await.1.unwrap(),
        json!({ "return_url": "https://app.example/back", "country": "de", "ttl_seconds": 3600 })
    );
    assert!(r.out.contains("https://openintents.io/portal/abc"));
}

#[tokio::test]
async fn an_unknown_subcommand_is_refused_without_calling_the_api() {
    let server = MockServer::start().await;
    let r = openintents(&server, &["customers", "explode"], "").await;
    assert_ne!(r.code, 0);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn sessions_open_names_the_customer_and_start_page() {
    let server = serve("POST", "/sessions", 201, session("open")).await;
    let r = openintents(
        &server,
        &[
            "sessions",
            "open",
            "--customer",
            "cus_1",
            "--start-url",
            "https://shop.example/",
            "--max-minutes",
            "5",
            "--country",
            "jp",
        ],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        sent(&server).await.1.unwrap(),
        json!({ "customer": "cus_1", "start_url": "https://shop.example/", "max_minutes": 5, "country": "jp" })
    );
    assert!(r.out.contains("ses_1"));
}

#[tokio::test]
async fn sessions_list_get_release_and_live() {
    let server = serve(
        "GET",
        "/sessions",
        200,
        json!({ "object": "list", "data": [session("open")] }),
    )
    .await;
    let r = openintents(
        &server,
        &["sessions", "list", "--status", "open", "--customer", "cus_1"],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(sent(&server).await.0, "GET /sessions?status=open&customer=cus_1");

    let server = serve("GET", "/sessions/ses_1", 200, session("open")).await;
    assert_eq!(openintents(&server, &["sessions", "get", "ses_1"], "").await.code, 0);

    let server = serve("POST", "/sessions/ses_1/release", 200, session("closed")).await;
    let r = openintents(&server, &["sessions", "release", "ses_1"], "").await;
    assert!(r.out.contains("closed"), "{}", r.out);

    let link = json!({ "object": "live_link", "session": "ses_1", "url": "https://openintents.io/live/xyz", "expires_at": "x" });
    let server = serve("POST", "/sessions/ses_1/live_links", 201, link).await;
    let r = openintents(&server, &["sessions", "live", "ses_1"], "").await;
    assert!(r.out.contains("https://openintents.io/live/xyz"), "{}", r.out);
}

#[tokio::test]
async fn sessions_command_sends_each_kind_of_command() {
    let cases: Vec<(Vec<&str>, Value)> = vec![
        (
            vec!["navigate", "https://shop.example/cart"],
            json!({ "type": "navigate", "url": "https://shop.example/cart" }),
        ),
        (vec!["snapshot"], json!({ "type": "snapshot" })),
        (vec!["read"], json!({ "type": "read" })),
        (vec!["read", "main"], json!({ "type": "read", "selector": "main" })),
        (
            vec!["click", "e12"],
            json!({ "type": "act", "ref": "e12", "action": "click" }),
        ),
        (
            vec!["type", "e3", "hello world"],
            json!({ "type": "act", "ref": "e3", "action": "type", "text": "hello world" }),
        ),
        (
            vec!["select", "e4", "M"],
            json!({ "type": "act", "ref": "e4", "action": "select", "value": "M" }),
        ),
        (
            vec!["press", "e4", "Enter"],
            json!({ "type": "act", "ref": "e4", "action": "press", "key": "Enter" }),
        ),
    ];
    for (words, body) in cases {
        let server = serve("POST", "/sessions/ses_1/commands", 200, command_result("done")).await;
        let mut args = vec!["sessions", "command", "ses_1"];
        args.extend(&words);
        let r = openintents(&server, &args, "").await;
        assert_eq!(r.code, 0, "{words:?}: {}", r.err);
        assert_eq!(sent(&server).await.1.unwrap(), body, "{words:?}");
    }
}

#[tokio::test]
async fn sessions_command_prints_the_snapshot_and_the_text() {
    let mut snap = command_result("done");
    snap["snapshot"] = json!("- button \"Buy\" [ref=e1]");
    snap["text"] = json!("Total 12.00");
    let server = serve("POST", "/sessions/ses_1/commands", 200, snap).await;
    let r = openintents(&server, &["sessions", "command", "ses_1", "snapshot"], "").await;
    assert!(
        r.out.contains("button \"Buy\" [ref=e1]") && r.out.contains("Total 12.00"),
        "{}",
        r.out
    );
}

#[tokio::test]
async fn sessions_command_exits_1_when_refused() {
    let mut refused = command_result("refused");
    refused["error"] = json!({ "code": "payment_not_allowed", "message": "sessions never pay" });
    let server = serve("POST", "/sessions/ses_1/commands", 200, refused).await;
    let r = openintents(&server, &["sessions", "command", "ses_1", "click", "e9"], "").await;
    assert_eq!(r.code, 1);
    assert!(r.out.contains("payment_not_allowed"), "{}", r.out);
}

#[tokio::test]
async fn sessions_screenshot_saves_the_jpeg() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sessions/ses_1/screenshot"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(vec![0xffu8, 0xd8, 0xff, 0xd9], "image/jpeg"))
        .mount(&server)
        .await;
    let file = std::env::temp_dir().join(format!("oi-shot-{}.jpg", uuid_like()));
    let r = openintents(
        &server,
        &["sessions", "screenshot", "ses_1", "--out", file.to_str().unwrap()],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(std::fs::read(&file).unwrap(), vec![0xff, 0xd8, 0xff, 0xd9]);
    assert_ne!(
        openintents(&server, &["sessions", "screenshot", "ses_1"], "")
            .await
            .code,
        0
    );
}

#[tokio::test]
async fn profiles_list_create_get_and_forget() {
    let server = serve(
        "GET",
        "/profiles",
        200,
        json!({ "object": "list", "data": [profile("idle")] }),
    )
    .await;
    let r = openintents(&server, &["profiles", "list", "--customer", "cus_1"], "").await;
    assert_eq!(sent(&server).await.0, "GET /profiles?customer=cus_1");
    assert!(r.out.contains("prf_1"));

    let server = serve("POST", "/profiles", 201, profile("idle")).await;
    assert_eq!(
        openintents(&server, &["profiles", "create", "--customer", "cus_1"], "")
            .await
            .code,
        0
    );
    assert_eq!(sent(&server).await.1.unwrap(), json!({ "customer": "cus_1" }));

    let server = serve("GET", "/profiles/prf_1", 200, profile("idle")).await;
    assert_eq!(openintents(&server, &["profiles", "get", "prf_1"], "").await.code, 0);

    let server = serve("DELETE", "/profiles/prf_1", 200, profile("forgotten")).await;
    let r = openintents(&server, &["profiles", "forget", "prf_1"], "").await;
    assert!(r.out.contains("forgotten"), "{}", r.out);
}

#[tokio::test]
async fn topups_create_takes_dollars_and_prints_the_link() {
    let topup = json!({ "id": "top_1", "object": "topup", "amount": 2500, "currency": "usd", "customer": "cus_1", "status": "open",
                        "url": "https://pay.example/top_1", "expires_at": "x", "created_at": "x" });
    let server = serve("POST", "/topups", 201, topup.clone()).await;
    let r = openintents(&server, &["topups", "create", "25", "--customer", "cus_1"], "").await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        sent(&server).await.1.unwrap(),
        json!({ "amount": 2500, "customer": "cus_1" })
    );
    assert!(
        r.out.contains("https://pay.example/top_1") && r.out.contains("$25.00"),
        "{}",
        r.out
    );

    let mut paid = topup;
    paid["status"] = json!("paid");
    let server = serve("GET", "/topups/top_1", 200, paid).await;
    assert!(openintents(&server, &["topups", "get", "top_1"], "")
        .await
        .out
        .contains("paid"));

    let server = MockServer::start().await;
    assert_ne!(openintents(&server, &["topups", "create", "lots"], "").await.code, 0);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn sign_ins_create_and_get() {
    let sign_in = json!({ "id": "sgn_1", "object": "sign_in", "status": "open", "customer": "cus_1", "shop": "amazon.com",
                          "country": "us", "country_source": "default", "return_url": null, "url": "https://openintents.io/signin/abc",
                          "test": true, "created_at": "x", "expires_at": "y", "ended_at": null });
    let server = serve("POST", "/sign_ins", 201, sign_in.clone()).await;
    let r = openintents(
        &server,
        &[
            "sign-ins",
            "create",
            "https://www.amazon.com/",
            "--customer",
            "cus_1",
            "--return-url",
            "https://app.example/",
        ],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        sent(&server).await.1.unwrap(),
        json!({ "shop_url": "https://www.amazon.com/", "customer": "cus_1", "return_url": "https://app.example/" })
    );
    assert!(r.out.contains("https://openintents.io/signin/abc"));

    let mut done = sign_in;
    done["status"] = json!("done");
    done["url"] = Value::Null;
    let server = serve("GET", "/sign_ins/sgn_1", 200, done).await;
    assert!(openintents(&server, &["sign-ins", "get", "sgn_1"], "")
        .await
        .out
        .contains("done"));
}

#[tokio::test]
async fn secrets_set_stores_without_printing_values() {
    let server = serve("POST", "/secrets", 201, secret()).await;
    let r = openintents(
        &server,
        &[
            "secrets",
            "set",
            "--kind",
            "card",
            "--label",
            "Work Visa",
            "--field",
            "number=4242424242424242",
            "--field",
            "cvc=123",
            "--shop",
            "airfrance.com",
            "--default",
        ],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        sent(&server).await.1.unwrap(),
        json!({ "kind": "card", "label": "Work Visa", "fields": { "number": "4242424242424242", "cvc": "123" },
                "shops": ["airfrance.com"], "default": true })
    );
    assert!(r.out.contains("Visa ••4242"));
    for text in [&r.out, &r.err] {
        assert!(!text.contains("4242424242424242") && !text.contains("123"), "{text}");
    }
}

#[tokio::test]
async fn secrets_set_reads_fields_from_stdin() {
    let server = serve("POST", "/secrets", 201, secret()).await;
    let r = openintents(
        &server,
        &["secrets", "set", "--kind", "card", "--label", "Work Visa", "--stdin"],
        r#"{"number":"4242424242424242"}"#,
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        sent(&server).await.1.unwrap()["fields"],
        json!({ "number": "4242424242424242" })
    );
    assert!(!r.out.contains("4242424242424242") && !r.err.contains("4242424242424242"));
}

#[tokio::test]
async fn secrets_set_does_not_repeat_a_value_in_an_error() {
    let server = MockServer::start().await;
    let r = openintents(
        &server,
        &[
            "secrets",
            "set",
            "--kind",
            "card",
            "--label",
            "x",
            "--field",
            "4242424242424242",
        ],
        "",
    )
    .await;
    assert_eq!(r.code, 1);
    assert!(!r.err.contains("4242424242424242"), "{}", r.err);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn secrets_list_get_delete_and_request() {
    let server = serve("GET", "/secrets", 200, json!({ "object": "list", "data": [secret()] })).await;
    let r = openintents(
        &server,
        &["secrets", "list", "--kind", "card", "--customer", "cus_1"],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("Work Visa") && r.out.contains("Visa ••4242"));
    let (line, _) = sent(&server).await;
    assert!(line.contains("customer=cus_1") && line.contains("kind=card"), "{line}");

    let server = serve("GET", "/secrets/sec_1", 200, secret()).await;
    assert_eq!(openintents(&server, &["secrets", "get", "sec_1"], "").await.code, 0);

    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/secrets/sec_1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    assert_eq!(openintents(&server, &["secrets", "delete", "sec_1"], "").await.code, 0);

    let created = json!({ "id": "sr_1", "object": "secret_request", "customer": "cus_1", "kinds": ["card", "address"],
                          "url": "https://openintents.io/secrets/abc", "expires_at": "x" });
    let server = serve("POST", "/customers/cus_1/secret_requests", 201, created).await;
    let r = openintents(
        &server,
        &[
            "secrets", "request", "cus_1", "--kind", "card", "--kind", "address", "--ttl", "3600",
        ],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        sent(&server).await.1.unwrap(),
        json!({ "kinds": ["card", "address"], "ttl_seconds": 3600 })
    );
    assert!(r.out.contains("https://openintents.io/secrets/abc"));
}

#[tokio::test]
async fn login_saves_the_pasted_key_for_later_commands() {
    let server = serve("GET", "/balance", 200, json!({ "available": 1000, "currency": "usd" })).await;
    let base = server.uri();
    let config = std::env::temp_dir().join(format!("oi-login-{}", uuid_like()));
    let config_for_login = config.clone();
    let (login, balance) = tokio::task::spawn_blocking(move || {
        let run = |args: &[&str], stdin: &str, key: Option<&str>| {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_openintents"));
            cmd.args(args)
                .env_remove("OPENINTENTS_API_KEY")
                .env("OPENINTENTS_BASE_URL", &base)
                .env("XDG_CONFIG_HOME", &config_for_login)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(k) = key {
                cmd.env("OPENINTENTS_API_KEY", k);
            }
            let mut child = cmd.spawn().unwrap();
            child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
            child.wait_with_output().unwrap()
        };
        (
            run(&["login", "--no-open"], " oi_test_abc123 \n", None),
            run(&["balance"], "", None),
        )
    })
    .await
    .unwrap();
    assert!(login.status.success(), "{}", String::from_utf8_lossy(&login.stderr));
    assert!(!String::from_utf8_lossy(&login.stdout).contains("oi_test_abc123"));
    let file = config.join("openintents").join("config.json");
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&file).unwrap()).unwrap(),
        json!({ "apiKey": "oi_test_abc123" })
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    }
    assert!(balance.status.success(), "{}", String::from_utf8_lossy(&balance.stderr));
    let seen = server.received_requests().await.unwrap();
    assert_eq!(
        seen[0].headers.get("authorization").unwrap().to_str().unwrap(),
        "Bearer oi_test_abc123"
    );
}

#[tokio::test]
async fn login_rejects_something_that_is_not_a_key() {
    let server = MockServer::start().await;
    let r = openintents(&server, &["login", "--no-open"], "hunter2\n").await;
    assert_eq!(r.code, 1);
    assert!(r.err.contains("oi_live_"), "{}", r.err);
}

#[tokio::test]
async fn run_and_list_still_work_and_take_context_and_status() {
    let intent = json!({ "id": "int_1", "object": "intent", "status": "queued", "intent": "flat white", "test": true, "created_at": "x" });
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intents"))
        .and(body_json(
            json!({ "intent": "flat white", "max_amount": 1000, "context": { "size": "large" } }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(intent.clone()))
        .mount(&server)
        .await;
    let r = openintents(
        &server,
        &["run", "flat white", "--max", "10", "--context", "size=large"],
        "",
    )
    .await;
    assert_eq!(r.code, 0, "{}", r.err);

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/intents"))
        .and(query_param("status", "failed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "object": "list", "data": [intent] })))
        .mount(&server)
        .await;
    let r = openintents(&server, &["list", "--status", "failed"], "").await;
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.contains("int_1"));
}

#[tokio::test]
async fn status_shows_a_payments_code_to_scan_and_answers_it_with_done() {
    let intent = json!({ "id": "int_1", "object": "intent", "status": "requires_input", "intent": "a lamp", "test": false,
        "created_at": "x", "input_request": { "id": "inp_1", "kind": "payment", "shop": "blinkit.com",
        "prompt": "Pay 249.00 INR", "url": "https://openintents.io/input/inp_1",
        "image_url": "https://openintents.io/input/inp_1/code.jpg", "expires_at": "x", "attempts_left": 1 } });
    let server = serve("GET", "/intents/int_1", 200, intent).await;
    let r = openintents(&server, &["status", "int_1"], "").await;
    assert!(
        r.out
            .contains("payment code to scan (theirs only): https://openintents.io/input/inp_1/code.jpg"),
        "{}{}",
        r.out,
        r.err
    );
    assert!(r.out.contains("--input inp_1 --done"), "{}", r.out);
}
