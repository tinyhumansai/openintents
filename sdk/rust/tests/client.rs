use std::time::Duration;

use openintents::{
    AnswerInput, Client, CreateIntent, CreateTopup, CustomerParams, Error, InputChannel, InputKind, Intent,
    IntentStatus, ListCustomers, ListIntents, SignInHint, TopupStatus, WaitOptions,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn intent(status: &str) -> serde_json::Value {
    json!({ "id": "int_1", "object": "intent", "status": status, "intent": "flat white",
            "test": true, "created_at": "2026-10-01T08:30:00Z" })
}

fn client(server: &MockServer) -> Client {
    Client::builder("oi_test_x")
        .base_url(server.uri())
        .max_retries(1)
        .build()
}

#[tokio::test]
async fn creates_an_intent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intents"))
        .and(header("authorization", "Bearer oi_test_x"))
        .and(header_exists("idempotency-key"))
        .and(body_json(json!({ "intent": "flat white", "max_amount": 1000 })))
        .respond_with(ResponseTemplate::new(201).set_body_json(intent("queued")))
        .expect(1)
        .mount(&server)
        .await;
    let it = client(&server)
        .create_intent(CreateIntent::new("flat white").max_amount(1000))
        .await
        .unwrap();
    assert_eq!(it.id, "int_1");
    assert_eq!(it.status, IntentStatus::Queued);
}

#[tokio::test]
async fn lists_with_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/intents"))
        .and(query_param("limit", "5"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "object": "list", "data": [intent("completed")], "next_cursor": null })),
        )
        .mount(&server)
        .await;
    let page = client(&server)
        .list_intents(&openintents::ListIntents {
            limit: Some(5),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page.data.len(), 1);
}

#[tokio::test]
async fn maps_api_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({ "error": { "type": "not_found", "message": "No such intent" } })),
        )
        .mount(&server)
        .await;
    match client(&server).get_intent("nope").await {
        Err(Error::Api {
            status, kind, message, ..
        }) => {
            assert_eq!(status, 404);
            assert_eq!(kind, "not_found");
            assert_eq!(message, "No such intent");
        }
        other => panic!("expected an API error, got {other:?}"),
    }
}

#[tokio::test]
async fn retries_server_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).insert_header("retry-after", "0.01"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("running")))
        .mount(&server)
        .await;
    assert_eq!(
        client(&server).get_intent("int_1").await.unwrap().status,
        IntentStatus::Running
    );
}

#[tokio::test]
async fn wait_returns_the_payment_link() {
    let server = MockServer::start().await;
    let mut needs_payment = intent("requires_payment");
    needs_payment["payment"] = json!({ "reason": "insufficient_balance", "payment_url": "https://pay/p",
        "total": 550, "fee": 28, "currency": "usd", "expires_at": "x" });
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("running")))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(needs_payment))
        .mount(&server)
        .await;
    let opts = WaitOptions {
        interval: Duration::from_millis(1),
        ..Default::default()
    };
    let it = client(&server).wait("int_1", opts).await.unwrap();
    assert_eq!(it.payment.unwrap().payment_url, "https://pay/p");
}

#[tokio::test]
async fn wait_returns_the_sign_in_link() {
    let server = MockServer::start().await;
    let mut needs_sign_in = intent("requires_login");
    needs_sign_in["sign_in"] = json!({ "url": "https://openintents.io/signin/sgn_1",
        "shop": "amazon.com", "expires_at": "x" });
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("running")))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(needs_sign_in))
        .mount(&server)
        .await;
    let opts = WaitOptions {
        interval: Duration::from_millis(1),
        ..Default::default()
    };
    let it = client(&server).wait("int_1", opts).await.unwrap();
    assert_eq!(it.status, IntentStatus::RequiresLogin);
    let sign_in = it.sign_in.unwrap();
    assert_eq!(
        (sign_in.url.as_str(), sign_in.shop.as_str()),
        ("https://openintents.io/signin/sgn_1", "amazon.com")
    );
}

#[tokio::test]
async fn wait_keeps_polling_past_a_sign_in_when_asked() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("requires_login")))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("completed")))
        .mount(&server)
        .await;
    let opts = WaitOptions {
        interval: Duration::from_millis(1),
        return_on_sign_in: false,
        ..Default::default()
    };
    let it = client(&server).wait("int_1", opts).await.unwrap();
    assert_eq!(it.status, IntentStatus::Completed);
}

#[test]
fn create_sends_sign_in_link() {
    let body = serde_json::to_value(CreateIntent::new("a lamp").sign_in_link(false)).unwrap();
    assert_eq!(body, json!({ "intent": "a lamp", "sign_in_link": false }));
}

#[tokio::test]
async fn wait_returns_the_input_request() {
    let server = MockServer::start().await;
    let mut needs_input = intent("requires_input");
    needs_input["input_request"] = json!({ "id": "inp_1", "kind": "code", "shop": "blinkit.com",
        "prompt": "Enter the code blinkit.com just sent to your phone.", "channel": "sms", "length": 6,
        "url": "https://openintents.io/input/inp_1", "expires_at": "x", "attempts_left": 2 });
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("running")))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(needs_input))
        .mount(&server)
        .await;
    let opts = WaitOptions {
        interval: Duration::from_millis(1),
        ..Default::default()
    };
    let it = client(&server).wait("int_1", opts).await.unwrap();
    assert_eq!(it.status, IntentStatus::RequiresInput);
    let request = it.input_request.unwrap();
    assert_eq!(
        (request.id.as_str(), request.kind, request.shop.as_str()),
        ("inp_1", InputKind::Code, "blinkit.com")
    );
    assert_eq!(
        (request.channel, request.length, request.attempts_left),
        (Some(InputChannel::Sms), Some(6), 2)
    );
    assert_eq!(request.prompt, "Enter the code blinkit.com just sent to your phone.");
    // An older response without `options` still parses.
    assert!(request.options.is_empty());
}

#[test]
fn a_detail_with_options_parses_and_round_trips() {
    let detail = json!({ "id": "inp_2", "kind": "detail", "shop": "flipkart.com",
        "prompt": "Which size would you like?", "options": ["S", "M", "L"], "choices": [],
        "fields": [], "expects": "text", "channel": null, "length": null, "url": "https://openintents.io/input/inp_2", "expires_at": "x", "attempts_left": 2 });
    let mut needs_input = intent("requires_input");
    needs_input["input_request"] = detail.clone();
    let it: Intent = serde_json::from_value(needs_input).unwrap();
    let request = it.input_request.unwrap();
    assert_eq!(request.kind, InputKind::Detail);
    assert_eq!(request.options, ["S", "M", "L"]);
    assert_eq!((request.channel, request.length), (None, None));
    assert_eq!(serde_json::to_value(&request).unwrap(), detail);
    assert_eq!(
        serde_json::to_value(AnswerInput::value(&request.id, "M")).unwrap(),
        json!({ "input": "inp_2", "value": "M" })
    );
}

#[tokio::test]
async fn wait_keeps_polling_past_an_input_request_when_asked() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("requires_input")))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("completed")))
        .mount(&server)
        .await;
    let opts = WaitOptions {
        interval: Duration::from_millis(1),
        return_on_input: false,
        ..Default::default()
    };
    let it = client(&server).wait("int_1", opts).await.unwrap();
    assert_eq!(it.status, IntentStatus::Completed);
}

#[test]
fn create_sends_sign_in_agent_and_hint() {
    let params = CreateIntent::new("a lamp")
        .sign_in_agent(false)
        .sign_in_hint(SignInHint::default().phone("+91 98765 43210"));
    assert_eq!(
        serde_json::to_value(params).unwrap(),
        json!({ "intent": "a lamp", "sign_in_agent": false, "sign_in_hint": { "phone": "+91 98765 43210" } })
    );
}

#[tokio::test]
async fn answers_an_input_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intents/int_1/input"))
        .and(header_exists("idempotency-key"))
        .and(body_json(json!({ "input": "inp_1", "value": "481516" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("running")))
        .expect(1)
        .mount(&server)
        .await;
    let it = client(&server)
        .answer_intent_input("int_1", AnswerInput::value("inp_1", "481516"))
        .await
        .unwrap();
    assert_eq!(it.status, IntentStatus::Running);
}

#[test]
fn answers_serialize_with_exactly_one_of_value_decline_and_done() {
    let to_json = |answer| serde_json::to_value(answer).unwrap();
    assert_eq!(
        to_json(AnswerInput::value("inp_1", "a@b.co")),
        json!({ "input": "inp_1", "value": "a@b.co" })
    );
    assert_eq!(
        to_json(AnswerInput::decline("inp_1")),
        json!({ "input": "inp_1", "decline": true })
    );
    assert_eq!(
        to_json(AnswerInput::done("inp_2")),
        json!({ "input": "inp_2", "done": true })
    );
}

#[tokio::test]
async fn an_answer_too_late_is_input_not_waiting() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intents/int_1/input"))
        .respond_with(ResponseTemplate::new(409).set_body_json(
            json!({ "error": { "type": "conflict", "code": "input_not_waiting", "message": "Expired." } }),
        ))
        .mount(&server)
        .await;
    match client(&server)
        .answer_intent_input("int_1", AnswerInput::decline("inp_1"))
        .await
    {
        Err(Error::Api { status, code, .. }) => {
            assert_eq!((status, code.as_deref()), (409, Some("input_not_waiting")));
        }
        other => panic!("expected an API error, got {other:?}"),
    }
}

#[tokio::test]
async fn wait_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("running")))
        .mount(&server)
        .await;
    let opts = WaitOptions {
        interval: Duration::from_millis(20),
        timeout: Duration::from_millis(1),
        ..Default::default()
    };
    assert!(matches!(
        client(&server).wait("int_1", opts).await,
        Err(Error::WaitTimeout { .. })
    ));
}

#[tokio::test]
async fn unknown_status_does_not_break_parsing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("some_future_status")))
        .mount(&server)
        .await;
    assert_eq!(
        client(&server).get_intent("int_1").await.unwrap().status,
        IntentStatus::Unknown
    );
}

#[tokio::test]
async fn gets_the_balance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/balance"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "object": "balance", "available": 24450, "currency": "usd" })),
        )
        .mount(&server)
        .await;
    assert_eq!(client(&server).balance().await.unwrap().available, 24450);
}

#[tokio::test]
async fn buys_for_a_customer_and_reads_their_balance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/balance"))
        .and(query_param("customer", "cus_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({ "object": "balance", "available": 0, "held": 450, "currency": "usd", "customer": "cus_1" }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/intents"))
        .and(body_json(json!({ "intent": "flat white", "customer": "cus_1" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(intent("queued")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/intents"))
        .and(query_param("customer", "cus_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "object": "list", "data": [] })))
        .expect(1)
        .mount(&server)
        .await;
    let oi = client(&server);
    let balance = oi.balance_for("cus_1").await.unwrap();
    assert_eq!((balance.held, balance.customer.as_deref()), (Some(450), Some("cus_1")));
    oi.create_intent(CreateIntent::new("flat white").customer("cus_1"))
        .await
        .unwrap();
    let only = ListIntents {
        customer: Some("cus_1".into()),
        ..Default::default()
    };
    assert!(oi.list_intents(&only).await.unwrap().data.is_empty());
}

#[tokio::test]
async fn creates_updates_and_lists_customers() {
    let server = MockServer::start().await;
    let customer = json!({ "id": "cus_1", "object": "customer", "external_id": "u-42", "name": null,
                           "email": null, "metadata": {}, "created_at": "x" });
    Mock::given(method("POST"))
        .and(path("/customers"))
        .and(header_exists("idempotency-key"))
        .and(body_json(json!({ "external_id": "u-42" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(customer.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/customers/cus_1"))
        .and(body_json(json!({ "name": "Ada", "email": "" })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "id": "cus_1", "object": "customer",
            "name": "Ada", "created_at": "x" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/customers"))
        .and(query_param("external_id", "u-42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "object": "list", "data": [customer.clone()] })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/customers/cus_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(customer))
        .expect(1)
        .mount(&server)
        .await;
    let oi = client(&server);
    let made = oi
        .create_customer(CustomerParams::default().external_id("u-42"))
        .await
        .unwrap();
    assert_eq!(made.external_id.as_deref(), Some("u-42"));
    let renamed = oi
        .update_customer("cus_1", CustomerParams::default().name("Ada").email(""))
        .await
        .unwrap();
    assert_eq!(renamed.name.as_deref(), Some("Ada"));
    let only = ListCustomers {
        external_id: Some("u-42".into()),
        ..Default::default()
    };
    assert_eq!(oi.list_customers(&only).await.unwrap().data.len(), 1);
    assert_eq!(oi.get_customer("cus_1").await.unwrap().id, "cus_1");
}

#[tokio::test]
async fn creates_and_retrieves_a_topup() {
    let server = MockServer::start().await;
    let topup = |status: &str| {
        json!({ "id": "pay_1", "object": "topup", "amount": 5000, "currency": "usd", "customer": "cus_1",
                "status": status, "url": "https://openintents.io/api/pay/pay_1", "expires_at": "x",
                "created_at": "x" })
    };
    Mock::given(method("POST"))
        .and(path("/topups"))
        .and(header_exists("idempotency-key"))
        .and(body_json(json!({ "amount": 5000, "customer": "cus_1" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(topup("open")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/topups/pay_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(topup("refunded")))
        .expect(1)
        .mount(&server)
        .await;
    let oi = client(&server);
    let made = oi.create_topup(CreateTopup::new(5000).customer("cus_1")).await.unwrap();
    assert_eq!(made.status, TopupStatus::Open);
    // A status this SDK does not know yet still parses.
    assert_eq!(oi.get_topup("pay_1").await.unwrap().status, TopupStatus::Unknown);
}
