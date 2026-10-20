use openintents::{
    Client, CreateProfile, CreateSession, Error, ListProfiles, ListSessions, ProfileStatus, SessionCommand,
    SessionCommandStatus, SessionStatus,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn client(server: &MockServer) -> Client {
    Client::builder("oi_test_x")
        .base_url(server.uri())
        .max_retries(1)
        .build()
}

fn profile(status: &str) -> serde_json::Value {
    json!({ "id": "prf_1", "object": "profile", "customer": "cus_1", "status": status, "lease": null,
            "size_bytes": null, "created_at": "2026-10-01T08:30:00Z", "last_used_at": null,
            "some_future_field": true })
}

fn session(status: &str) -> serde_json::Value {
    json!({ "id": "ses_1", "object": "session", "profile": "prf_1", "customer": "cus_1", "status": status,
            "test": true, "created_at": "2026-10-01T08:30:00Z", "expires_at": "2026-10-01T08:45:00Z" })
}

fn command(kind: &str, status: &str) -> serde_json::Value {
    json!({ "id": "cmd_1", "object": "session_command", "type": kind, "status": status,
            "created_at": "2026-10-01T08:30:00Z" })
}

fn no_idempotency_key(req: &Request) -> bool {
    !req.headers.contains_key("idempotency-key")
}

#[tokio::test]
async fn lists_makes_retrieves_and_forgets_profiles() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/profiles"))
        .and(query_param("customer", "cus_1"))
        .and(query_param("limit", "5"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "object": "list", "data": [profile("in_use")] })),
        )
        .expect(1)
        .mount(&server)
        .await;
    // Making a profile is idempotent by itself: no key.
    Mock::given(method("POST"))
        .and(path("/profiles"))
        .and(body_json(json!({ "customer": "cus_1" })))
        .and(no_idempotency_key)
        .respond_with(ResponseTemplate::new(201).set_body_json(profile("ready")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/profiles"))
        .and(body_json(json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(profile("ready")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/profiles/prf_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(profile("being_audited")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/profiles/prf_1"))
        .respond_with(ResponseTemplate::new(202).set_body_json(profile("forgetting")))
        .expect(1)
        .mount(&server)
        .await;

    let oi = client(&server);
    let only = ListProfiles {
        customer: Some("cus_1".into()),
        limit: Some(5),
        ..Default::default()
    };
    let page = oi.list_profiles(&only).await.unwrap();
    assert_eq!(page.data[0].status, ProfileStatus::InUse);
    let made = oi
        .create_profile(CreateProfile::default().customer("cus_1"))
        .await
        .unwrap();
    assert_eq!(made.customer.as_deref(), Some("cus_1"));
    oi.create_profile(CreateProfile::default()).await.unwrap();
    // A status this SDK does not know yet still parses.
    assert_eq!(oi.get_profile("prf_1").await.unwrap().status, ProfileStatus::Unknown);
    assert_eq!(
        oi.forget_profile("prf_1").await.unwrap().status,
        ProfileStatus::Forgetting
    );
}

#[tokio::test]
async fn opens_lists_retrieves_and_releases_sessions() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/sessions"))
        .and(header_exists("idempotency-key"))
        .and(body_json(
            json!({ "customer": "cus_1", "start_url": "https://shop.example/login", "max_minutes": 10 }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(session("queued")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sessions"))
        .and(query_param("status", "open"))
        .and(query_param("profile", "prf_1"))
        .and(query_param("customer", "cus_1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "object": "list", "data": [session("open")], "next_cursor": "ses_1" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sessions/ses_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(session("paused")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/sessions/ses_1/release"))
        .and(header_exists("idempotency-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(session("closing")))
        .expect(1)
        .mount(&server)
        .await;

    let oi = client(&server);
    let opened = oi
        .create_session(
            CreateSession::default()
                .customer("cus_1")
                .start_url("https://shop.example/login")
                .max_minutes(10),
        )
        .await
        .unwrap();
    assert_eq!(opened.status, SessionStatus::Queued);
    assert!(opened.test);
    let open = ListSessions {
        status: Some(SessionStatus::Open),
        profile: Some("prf_1".into()),
        customer: Some("cus_1".into()),
        ..Default::default()
    };
    let page = oi.list_sessions(&open).await.unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some("ses_1"));
    assert_eq!(oi.get_session("ses_1").await.unwrap().status, SessionStatus::Unknown);
    let released = oi.release_session("ses_1").await.unwrap();
    assert_eq!(released.status, SessionStatus::Closing);
    assert!(!released.status.is_final());
}

#[tokio::test]
async fn runs_commands_done_pending_and_refused() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/sessions/ses_1/commands"))
        .and(header_exists("idempotency-key"))
        .and(body_json(
            json!({ "type": "act", "ref": "e12", "action": "type", "text": "ada@example.com" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(command("act", "done")))
        .expect(1)
        .mount(&server)
        .await;
    // A 202 still returns the command, `pending`.
    Mock::given(method("POST"))
        .and(path("/sessions/ses_1/commands"))
        .and(header("idempotency-key", "nav-1"))
        .and(body_json(
            json!({ "type": "navigate", "url": "https://shop.example/cart" }),
        ))
        .respond_with(ResponseTemplate::new(202).set_body_json(command("navigate", "pending")))
        .expect(1)
        .mount(&server)
        .await;
    let mut refused = command("act", "refused");
    refused["error"] = json!({ "code": "payment_not_allowed", "message": "A session never pays." });
    Mock::given(method("POST"))
        .and(path("/sessions/ses_1/commands"))
        .and(body_json(json!({ "type": "act", "ref": "e40", "action": "click" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(refused))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sessions/ses_1/commands/cmd_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(command("navigate", "done")))
        .expect(1)
        .mount(&server)
        .await;

    let oi = client(&server);
    let typed = oi
        .run_session_command("ses_1", SessionCommand::type_text("e12", "ada@example.com"))
        .await
        .unwrap();
    assert_eq!((typed.kind.as_str(), typed.status), ("act", SessionCommandStatus::Done));
    let pending = oi
        .run_session_command_idempotent("ses_1", SessionCommand::navigate("https://shop.example/cart"), "nav-1")
        .await
        .unwrap();
    assert_eq!(pending.status, SessionCommandStatus::Pending);
    let done = oi.get_session_command("ses_1", "cmd_1").await.unwrap();
    assert_eq!(done.status, SessionCommandStatus::Done);
    let no = oi
        .run_session_command("ses_1", SessionCommand::click("e40"))
        .await
        .unwrap();
    assert_eq!(no.status, SessionCommandStatus::Refused);
    assert_eq!(no.error.unwrap().code, "payment_not_allowed");
}

#[test]
fn serializes_every_command() {
    let cases = [
        (
            SessionCommand::navigate("https://shop.example/"),
            json!({ "type": "navigate", "url": "https://shop.example/" }),
        ),
        (SessionCommand::snapshot(), json!({ "type": "snapshot" })),
        (
            SessionCommand::click("e1"),
            json!({ "type": "act", "ref": "e1", "action": "click" }),
        ),
        (
            SessionCommand::type_text("e2", "line one\nline two"),
            json!({ "type": "act", "ref": "e2", "action": "type", "text": "line one\nline two" }),
        ),
        (
            SessionCommand::select("e3", "Large"),
            json!({ "type": "act", "ref": "e3", "action": "select", "value": "Large" }),
        ),
        (
            SessionCommand::press("e4", "Shift+Tab"),
            json!({ "type": "act", "ref": "e4", "action": "press", "key": "Shift+Tab" }),
        ),
        (SessionCommand::read(), json!({ "type": "read" })),
        (
            SessionCommand::read_selector("main h1"),
            json!({ "type": "read", "selector": "main h1" }),
        ),
    ];
    for (command, expected) in cases {
        assert_eq!(serde_json::to_value(&command).unwrap(), expected);
    }
}

#[tokio::test]
async fn returns_a_screenshot_as_jpeg_bytes() {
    let server = MockServer::start().await;
    let jpeg = vec![0xff, 0xd8, 0xff, 0xe0];
    Mock::given(method("GET"))
        .and(path("/sessions/ses_1/screenshot"))
        .and(header("accept", "image/jpeg"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(jpeg.clone(), "image/jpeg"))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(client(&server).session_screenshot("ses_1").await.unwrap(), jpeg);
}

#[tokio::test]
async fn errors_carry_the_lease_and_code() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/sessions"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({ "error": {
            "type": "conflict", "code": "profile_busy", "message": "Profile in use.",
            "lease": { "holder": { "type": "intent", "id": "int_1" }, "expires_at": "2026-10-01T08:31:00Z" }
        } })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sessions/ses_1/screenshot"))
        .respond_with(ResponseTemplate::new(409).set_body_json(
            json!({ "error": { "type": "conflict", "code": "session_not_open", "message": "Not open." } }),
        ))
        .mount(&server)
        .await;

    let oi = client(&server);
    match oi.create_session(CreateSession::default()).await {
        Err(Error::Api {
            status,
            code,
            lease: Some(lease),
            ..
        }) => {
            assert_eq!((status, code.as_deref()), (409, Some("profile_busy")));
            assert_eq!(
                (lease.holder.kind.as_str(), lease.holder.id.as_str()),
                ("intent", "int_1")
            );
            assert_eq!(lease.expires_at, "2026-10-01T08:31:00Z");
        }
        other => panic!("expected profile_busy with a lease, got {other:?}"),
    }
    match oi.session_screenshot("ses_1").await {
        Err(Error::Api { code, lease, .. }) => {
            assert_eq!(code.as_deref(), Some("session_not_open"));
            assert!(lease.is_none());
        }
        other => panic!("expected session_not_open, got {other:?}"),
    }
}
