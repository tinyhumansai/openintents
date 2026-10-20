//! The v1 surface added since the first release: country, waiting, forms,
//! secrets, sign-ins, live links, portal sessions and webhook endpoints.

use openintents::{
    AnswerInput, Client, CountrySource, CreateIntent, CreatePortalSession, CreateSecret, CreateSecretRequest,
    CreateSession, CreateSignIn, CreateWebhookEndpoint, CustomerParams, DeliveryStatus, Error, InputExpects,
    InputFieldType, InputKind, Intent, LimitsParams, ListSecrets, Page, SecretKind, SignInStatus,
    UpdateWebhookEndpoint, WebhookData, WebhookEvent,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer) -> Client {
    Client::builder("oi_test_x")
        .base_url(server.uri())
        .max_retries(1)
        .build()
}

fn intent(status: &str) -> serde_json::Value {
    json!({ "id": "int_1", "object": "intent", "status": status, "intent": "flat white",
            "test": true, "created_at": "2026-10-01T08:30:00Z" })
}

#[test]
fn create_intent_sends_country_secrets_and_webhook() {
    let body = serde_json::to_value(
        CreateIntent::new("book a flight")
            .country("fr")
            .end_user_ip("203.0.113.9")
            .webhook_url("https://h.example/i")
            .input_ttl_seconds(3600)
            .secrets(["sec_1"])
            .shops(["airfrance.com"]),
    )
    .unwrap();
    assert_eq!(
        body,
        json!({ "intent": "book a flight", "country": "fr", "end_user_ip": "203.0.113.9",
                "webhook_url": "https://h.example/i", "input_ttl_seconds": 3600,
                "secrets": ["sec_1"], "shops": ["airfrance.com"] })
    );
}

#[test]
fn intents_and_sessions_carry_their_country_and_webhook() {
    let i: Intent = serde_json::from_value(json!({
        "id": "int_1", "status": "queued", "intent": "x", "test": true, "created_at": "t",
        "country": "de", "country_source": "end_user_ip", "webhook_url": "https://h.example/i",
        "webhook_secret": "whsec_1", "secrets": ["sec_1"], "shops": ["a.com"],
        "country_source_future": 1
    }))
    .unwrap();
    assert_eq!(i.country.as_deref(), Some("de"));
    assert_eq!(i.country_source, Some(CountrySource::EndUserIp));
    assert_eq!(i.webhook_secret.as_deref(), Some("whsec_1"));
    assert_eq!(i.shops, ["a.com"]);

    let s = serde_json::to_value(
        CreateSession::default()
            .country("jp")
            .webhook_url("https://h.example/s"),
    )
    .unwrap();
    assert_eq!(s, json!({ "country": "jp", "webhook_url": "https://h.example/s" }));

    let other: Intent = serde_json::from_value(
        json!({ "id": "i", "status": "queued", "intent": "x", "created_at": "t", "country_source": "from_mars" }),
    )
    .unwrap();
    assert_eq!(other.country_source, Some(CountrySource::Unknown));
}

#[tokio::test]
async fn create_intent_waiting_asks_the_api_to_hold_the_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intents"))
        .and(header("Prefer", "wait=30"))
        .and(header_exists("idempotency-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("completed")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/intents/int_1"))
        .and(header("Prefer", "wait=5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("running")))
        .expect(1)
        .mount(&server)
        .await;

    let oi = client(&server);
    let done = oi
        .create_intent_waiting(CreateIntent::new("flat white"), 30)
        .await
        .unwrap();
    assert_eq!(done.status.as_str(), "completed");
    oi.get_intent_waiting("int_1", 5).await.unwrap();
}

#[tokio::test]
async fn a_held_request_outlasts_the_client_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/intents/int_1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(intent("running"))
                .set_delay(std::time::Duration::from_millis(400)),
        )
        .mount(&server)
        .await;
    let oi = Client::builder("oi_test_x")
        .base_url(server.uri())
        .timeout(std::time::Duration::from_millis(100))
        .max_retries(0)
        .build();
    assert!(matches!(oi.get_intent("int_1").await, Err(Error::Network(_))));
    assert!(oi.get_intent_waiting("int_1", 1).await.is_ok());
}

#[test]
fn questions_offer_choices_and_forms_and_are_answered_with_values() {
    let q: openintents::InputRequest = serde_json::from_value(json!({
        "id": "inp_1", "kind": "detail", "shop": "airline.com", "prompt": "Pick a seat", "options": [],
        "choices": [{ "id": "seat_12a", "label": "12A", "description": "Window", "price_delta": 1500, "currency": "usd" },
                    { "id": "seat_12b", "label": "12B" }],
        "fields": [{ "name": "row", "label": "Row", "type": "choice", "required": true,
                     "options": [{ "id": "a", "label": "A" }, { "id": "b", "label": "B" }] },
                   { "name": "dob", "label": "Born", "type": "date", "required": false }],
        "expects": "choice", "channel": null, "length": null, "url": "https://x/input/inp_1",
        "expires_at": "t", "attempts_left": 2
    }))
    .unwrap();
    assert_eq!(q.choices[0].price_delta, Some(1500));
    assert_eq!(q.choices[1].description, None);
    assert_eq!(q.fields[0].kind, InputFieldType::Choice);
    assert_eq!(q.fields[0].options.len(), 2);
    assert_eq!(q.fields[1].kind, InputFieldType::Date);
    assert_eq!(q.expects, Some(InputExpects::Choice));

    let sensitive: openintents::InputRequest = serde_json::from_value(json!({
        "id": "inp_2", "kind": "sensitive", "shop": "airline.com", "prompt": "Passport", "url": "u",
        "expires_at": "t", "attempts_left": 1
    }))
    .unwrap();
    assert_eq!(sensitive.kind, InputKind::Sensitive);
    assert!(sensitive.choices.is_empty() && sensitive.fields.is_empty());

    let answer = serde_json::to_value(AnswerInput::values("inp_1", [("row", "a"), ("dob", "1990-01-02")])).unwrap();
    assert_eq!(
        answer,
        json!({ "input": "inp_1", "values": { "row": "a", "dob": "1990-01-02" } })
    );
}

#[tokio::test]
async fn a_sensitive_question_refuses_an_answer_off_the_page() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intents/int_1/input"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "error": { "type": "invalid_request", "code": "answer_on_page", "message": "answer on the page" }
        })))
        .mount(&server)
        .await;
    let err = client(&server)
        .answer_intent_input("int_1", AnswerInput::value("inp_2", "X123"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Api { status: 422, code: Some(ref c), .. } if c == "answer_on_page"));
}

#[tokio::test]
async fn a_429_carries_retry_after_and_the_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/customers/cus_1"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "0")
                .insert_header("RateLimit-Limit", "300")
                .insert_header("RateLimit-Remaining", "0")
                .insert_header("RateLimit-Reset", "7")
                .set_body_json(json!({
                    "error": { "type": "rate_limit", "code": "rate_limit_exceeded", "message": "slow down" }
                })),
        )
        .mount(&server)
        .await;
    match client(&server).get_customer("cus_1").await.unwrap_err() {
        Error::Api {
            status,
            code,
            retry_after,
            rate_limit,
            ..
        } => {
            assert_eq!(status, 429);
            assert_eq!(code.as_deref(), Some("rate_limit_exceeded"));
            assert_eq!(retry_after, Some(0));
            let r = rate_limit.unwrap();
            assert_eq!((r.limit, r.remaining, r.reset), (300, 0, 7));
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn customers_carry_limits() {
    let params = serde_json::to_value(
        CustomerParams::default().limits(LimitsParams::default().per_intent(Some(5000)).monthly(None)),
    )
    .unwrap();
    assert_eq!(params, json!({ "limits": { "per_intent": 5000, "monthly": null } }));

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/customers/cus_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "cus_1", "created_at": "t", "limits": { "per_intent": 5000, "monthly": null }
        })))
        .mount(&server)
        .await;
    let oi = client(&server);
    let limits = oi.get_customer("cus_1").await.unwrap().limits.unwrap();
    assert_eq!((limits.per_intent, limits.monthly), (Some(5000), None));
}

#[tokio::test]
async fn makes_portal_sessions_live_links_and_secret_requests() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/customers/cus_1/portal_sessions"))
        .and(header_exists("idempotency-key"))
        .and(body_json(
            json!({ "return_url": "https://me.example", "ttl_seconds": 600 }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "pts_1", "object": "portal_session", "customer": "cus_1", "country": "fr",
            "country_source": "request_ip", "url": "https://x/portal/pts_1", "expires_at": "t"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/sessions/ses_1/live_links"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "object": "live_link", "session": "ses_1", "url": "https://x/live/1", "expires_at": "t"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/customers/cus_1/secret_requests"))
        .and(body_json(json!({ "kinds": ["contact", "identity_document"] })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "sqr_1", "object": "secret_request", "customer": "cus_1",
            "kinds": ["contact", "identity_document"], "url": "https://x/secrets/sqr_1", "expires_at": "t"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/secret_requests/sqr_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "sqr_1", "object": "secret_request", "customer": "cus_1", "mode": "test",
            "kinds": ["contact"], "state": "done", "expires_at": "t", "completed_at": "t2", "secrets": ["sec_9"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let oi = client(&server);
    let portal = oi
        .create_portal_session(
            "cus_1",
            CreatePortalSession::default()
                .return_url("https://me.example")
                .ttl_seconds(600),
        )
        .await
        .unwrap();
    assert_eq!(portal.country_source, CountrySource::RequestIp);
    assert_eq!(oi.create_live_link("ses_1").await.unwrap().url, "https://x/live/1");
    let made = oi
        .create_secret_request(
            "cus_1",
            CreateSecretRequest::new([SecretKind::Contact, SecretKind::IdentityDocument]),
        )
        .await
        .unwrap();
    assert_eq!(made.kinds.len(), 2);
    assert_eq!(oi.get_secret_request("sqr_1").await.unwrap().secrets, ["sec_9"]);
}

fn secret() -> serde_json::Value {
    json!({ "id": "sec_1", "object": "secret", "customer": null, "kind": "loyalty", "label": "Flying Blue",
            "mask": "••7890", "public": { "program": "Flying Blue" }, "shops": ["a.com"], "default": true,
            "mode": "test", "entered_by": "developer", "version": 1, "created_at": "t", "updated_at": "t",
            "expires_at": null, "last_used_at": null })
}

#[tokio::test]
async fn stores_lists_replaces_and_deletes_secrets() {
    let server = MockServer::start().await;
    let body = json!({ "kind": "loyalty", "label": "Flying Blue", "fields": { "member_number": "1234567890" },
                       "shops": ["a.com"], "default": true });
    Mock::given(method("POST"))
        .and(path("/secrets"))
        .and(header_exists("idempotency-key"))
        .and(body_json(body.clone()))
        .respond_with(ResponseTemplate::new(201).set_body_json(secret()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/secrets/sec_1"))
        .and(header_exists("idempotency-key"))
        .and(body_json(body))
        .respond_with(ResponseTemplate::new(200).set_body_json(secret()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/secrets"))
        .and(query_param("customer", "cus_1"))
        .and(query_param("kind", "loyalty"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "object": "list", "data": [secret()] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/secrets/sec_1/uses"))
        .and(query_param("limit", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list", "next_cursor": "c2",
            "data": [{ "id": "use_1", "object": "secret_use", "secret": "sec_1", "kind": "loyalty",
                       "action": "refuse", "actor": "worker", "outcome": "refused",
                       "reason": "shop_not_allowed", "shop": "b.com", "at": "t" }]
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/secrets/sec_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(secret()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/secrets/sec_1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let oi = client(&server);
    let new = || {
        CreateSecret::new(SecretKind::Loyalty, "Flying Blue", [("member_number", "1234567890")])
            .shops(["a.com"])
            .default(true)
    };
    let made = oi.create_secret(new()).await.unwrap();
    assert_eq!((made.mask.as_str(), made.version), ("••7890", 1));
    oi.replace_secret("sec_1", new()).await.unwrap();
    let list = oi
        .list_secrets(&ListSecrets {
            customer: Some("cus_1".into()),
            kind: Some(SecretKind::Loyalty),
        })
        .await
        .unwrap();
    assert_eq!(list.data.len(), 1);
    let uses = oi
        .list_secret_uses(
            "sec_1",
            &Page {
                limit: Some(2),
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(uses.data[0].reason.as_deref(), Some("shop_not_allowed"));
    assert_eq!(uses.next_cursor.as_deref(), Some("c2"));
    oi.get_secret("sec_1").await.unwrap();
    oi.delete_secret("sec_1").await.unwrap();
}

#[tokio::test]
async fn makes_and_reads_sign_ins() {
    let server = MockServer::start().await;
    let sign_in = json!({ "id": "sgn_1", "object": "sign_in", "status": "open", "customer": "cus_1",
        "shop": "amazon.com", "country": "us", "country_source": "default", "url": "https://x/signin/sgn_1",
        "test": true, "created_at": "t", "expires_at": "t2", "ended_at": null });
    Mock::given(method("POST"))
        .and(path("/sign_ins"))
        .and(header_exists("idempotency-key"))
        .and(body_json(
            json!({ "shop_url": "https://www.amazon.com/", "customer": "cus_1" }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(sign_in.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sign_ins/sgn_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sign_in))
        .expect(1)
        .mount(&server)
        .await;
    let oi = client(&server);
    let made = oi
        .create_sign_in(CreateSignIn::new("https://www.amazon.com/").customer("cus_1"))
        .await
        .unwrap();
    assert_eq!((made.status, made.shop.as_str()), (SignInStatus::Open, "amazon.com"));
    assert_eq!(
        oi.get_sign_in("sgn_1").await.unwrap().url.as_deref(),
        Some("https://x/signin/sgn_1")
    );
}

#[tokio::test]
async fn manages_webhook_endpoints_and_their_deliveries() {
    let server = MockServer::start().await;
    let endpoint = |secret: bool| {
        let mut e = json!({ "id": "whe_1", "object": "webhook_endpoint", "url": "https://h.example/hook",
            "events": ["intent.completed"], "disabled": false, "created_at": "t" });
        if secret {
            e["secret"] = json!("whsec_new");
            e["previous_secret_expires_at"] = json!("t2");
        }
        e
    };
    let delivery = json!({ "id": "whd_1", "object": "webhook_delivery", "endpoint": "whe_1", "event_id": "evt_1",
        "event_type": "webhook_endpoint.test", "status": "delivered", "attempts": 1, "response_code": 200,
        "created_at": "t", "delivered_at": "t" });
    Mock::given(method("POST"))
        .and(path("/webhook_endpoints"))
        .and(header_exists("idempotency-key"))
        .and(body_json(
            json!({ "url": "https://h.example/hook", "events": ["intent.completed"] }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(endpoint(true)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/webhook_endpoints"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [endpoint(false)] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/webhook_endpoints/whe_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(endpoint(false)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/webhook_endpoints/whe_1"))
        .and(body_json(json!({ "events": [], "disabled": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(endpoint(false)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/webhook_endpoints/whe_1/rotate_secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(endpoint(true)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/webhook_endpoints/whe_1/test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(delivery.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/webhook_endpoints/whe_1/deliveries"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [delivery.clone()] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/webhook_endpoints/whe_1/deliveries/whd_1/redeliver"))
        .respond_with(ResponseTemplate::new(200).set_body_json(delivery))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/webhook_endpoints/whe_1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let oi = client(&server);
    let made = oi
        .create_webhook_endpoint(CreateWebhookEndpoint::new("https://h.example/hook").events(["intent.completed"]))
        .await
        .unwrap();
    assert_eq!(made.secret.as_deref(), Some("whsec_new"));
    let page = Page {
        limit: Some(10),
        cursor: None,
    };
    assert_eq!(oi.list_webhook_endpoints(&page).await.unwrap().data.len(), 1);
    assert!(oi.get_webhook_endpoint("whe_1").await.unwrap().secret.is_none());
    oi.update_webhook_endpoint(
        "whe_1",
        UpdateWebhookEndpoint::default()
            .events(Vec::<String>::new())
            .disabled(true),
    )
    .await
    .unwrap();
    let rotated = oi.rotate_webhook_secret("whe_1").await.unwrap();
    assert_eq!(rotated.previous_secret_expires_at.as_deref(), Some("t2"));
    let tested = oi.test_webhook_endpoint("whe_1").await.unwrap();
    assert_eq!(
        (tested.status, tested.response_code),
        (DeliveryStatus::Delivered, Some(200))
    );
    assert_eq!(
        oi.list_webhook_deliveries("whe_1", &Page::default())
            .await
            .unwrap()
            .data
            .len(),
        1
    );
    oi.redeliver_webhook("whe_1", "whd_1").await.unwrap();
    oi.delete_webhook_endpoint("whe_1").await.unwrap();
}

fn event(kind: &str, data: serde_json::Value) -> WebhookEvent {
    serde_json::from_value(json!({ "id": "evt_1", "type": kind, "created_at": "t", "data": data })).unwrap()
}

#[test]
fn sign_in_secret_request_and_test_events_parse() {
    let e = event(
        "sign_in.completed",
        json!({ "id": "sgn_1", "status": "done", "shop": "amazon.com", "created_at": "t", "expires_at": "t2" }),
    );
    assert_eq!(e.sign_in().unwrap().status, SignInStatus::Done);
    assert!(e.intent().is_none());

    let e = event(
        "secret_request.completed",
        json!({ "id": "sqr_1", "customer": "cus_1", "mode": "live", "kinds": ["address"], "state": "done",
                "expires_at": "t", "secrets": ["sec_1"] }),
    );
    assert_eq!(e.secret_request().unwrap().secrets, ["sec_1"]);

    let e = event(
        "webhook_endpoint.test",
        json!({ "endpoint": "whe_1", "message": "hello" }),
    );
    assert!(matches!(e.data, WebhookData::Other(_)));
}
