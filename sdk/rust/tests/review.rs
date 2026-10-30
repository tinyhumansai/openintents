//! The order review (`review_before_order`), read from the shared wire
//! examples in `spec/examples`.

use std::path::PathBuf;

use openintents::{
    AnswerInput, Client, CreateIntent, InputKind, IntentStatus, OrderFee, OrderPaymentMethod, OrderReview, ReceiptItem,
};
use serde_json::{json, Value};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn example(name: &str) -> Value {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/examples")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(&file).expect("read example")).expect("parse example")
}

fn client(server: &MockServer) -> Client {
    Client::builder("oi_test_x")
        .base_url(server.uri())
        .max_retries(0)
        .build()
}

fn the_review() -> OrderReview {
    OrderReview {
        merchant: "blinkit.com".into(),
        items: vec![
            ReceiptItem {
                name: "Protein bar, sugar free".into(),
                quantity: 2,
                amount: 10000,
            },
            ReceiptItem {
                name: "Roasted chana".into(),
                quantity: 1,
                amount: 4500,
            },
        ],
        fees: vec![
            OrderFee {
                label: "Delivery fee".into(),
                amount: 2500,
            },
            OrderFee {
                label: "Cash on delivery fee".into(),
                amount: 1000,
            },
        ],
        total: 18000,
        currency: "inr".into(),
        deliver_to: Some("Home, 12 MG Road, Bengaluru 560001".into()),
        eta: Some("12 minutes".into()),
        payment_method: OrderPaymentMethod::CashOnDelivery,
    }
}

#[test]
fn create_intent_sends_review_before_order_only_when_set() {
    let on = serde_json::to_value(CreateIntent::new("snacks").pay_at_shop(true).review_before_order(true)).unwrap();
    assert_eq!(
        on,
        json!({ "intent": "snacks", "pay_at_shop": true, "review_before_order": true })
    );
    let off = serde_json::to_value(CreateIntent::new("snacks")).unwrap();
    assert_eq!(off, json!({ "intent": "snacks" }));
}

#[tokio::test]
async fn a_review_intent_parses_from_the_wire() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/intents"))
        .and(body_json(json!({ "intent": "snacks", "review_before_order": true })))
        .respond_with(ResponseTemplate::new(201).set_body_json(example("intent-requires-review.json")))
        .mount(&server)
        .await;
    let got = client(&server)
        .create_intent(CreateIntent::new("snacks").review_before_order(true))
        .await
        .unwrap();
    assert_eq!(got.status, IntentStatus::RequiresInput);
    assert!(got.review_before_order);
    let request = got.input_request.unwrap();
    assert_eq!(request.kind, InputKind::Review);
    assert_eq!(request.review, Some(the_review()));
    assert_eq!(request.attempts_left, 2);
}

#[tokio::test]
async fn a_review_is_answered_done_with_a_change_or_declined() {
    let server = MockServer::start().await;
    let mut running = example("intent-requires-review.json");
    running["status"] = json!("running");
    running["input_request"] = Value::Null;
    for answer in [
        json!({ "input": "inp_review_1", "done": true }),
        json!({ "input": "inp_review_1", "value": "remove the chana" }),
    ] {
        Mock::given(method("POST"))
            .and(path("/intents/int_review_1/input"))
            .and(body_json(answer))
            .respond_with(ResponseTemplate::new(200).set_body_json(running.clone()))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/intents/int_review_1/input"))
        .and(body_json(json!({ "input": "inp_review_1", "decline": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(example("intent-review-declined.json")))
        .expect(1)
        .mount(&server)
        .await;
    let oi = client(&server);
    let placed = oi
        .answer_intent_input("int_review_1", AnswerInput::done("inp_review_1"))
        .await
        .unwrap();
    assert_eq!(placed.status, IntentStatus::Running);
    oi.answer_intent_input("int_review_1", AnswerInput::value("inp_review_1", "remove the chana"))
        .await
        .unwrap();
    let declined = oi
        .answer_intent_input("int_review_1", AnswerInput::decline("inp_review_1"))
        .await
        .unwrap();
    assert_eq!(declined.status, IntentStatus::Failed);
    assert_eq!(declined.error.unwrap().code, "order_declined");
}

#[test]
fn the_receipt_carries_the_approved_address() {
    let got: openintents::Intent = serde_json::from_value(example("intent-completed-reviewed.json")).unwrap();
    let receipt = got.result.unwrap();
    assert_eq!(
        receipt.deliver_to.as_deref(),
        Some("Home, 12 MG Road, Bengaluru 560001")
    );
    assert_eq!(receipt.paid_with.as_deref(), Some("cash_on_delivery"));
}

#[test]
fn unknown_kinds_and_payment_methods_still_parse() {
    let mut body = example("intent-requires-review.json");
    body["input_request"]["kind"] = json!("something_new");
    body["input_request"]["review"]["payment_method"] = json!("something_else");
    let got: openintents::Intent = serde_json::from_value(body).unwrap();
    let request = got.input_request.unwrap();
    assert_eq!(request.kind, InputKind::Unknown);
    assert_eq!(request.review.unwrap().payment_method, OrderPaymentMethod::Unknown);
}

#[test]
fn an_intent_without_the_new_fields_still_parses() {
    let got: openintents::Intent = serde_json::from_value(json!({
        "id": "int_1", "object": "intent", "status": "completed", "intent": "a lamp", "test": true,
        "created_at": "x", "result": { "merchant": "m", "total": 1, "fee": 0, "currency": "usd" }
    }))
    .unwrap();
    assert!(!got.review_before_order);
    assert_eq!(got.result.unwrap().deliver_to, None);
}
