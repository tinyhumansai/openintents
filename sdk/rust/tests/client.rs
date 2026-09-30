use std::time::Duration;

use openintents::{Client, CreateIntent, Error, IntentStatus, WaitOptions};
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
async fn wait_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(intent("running")))
        .mount(&server)
        .await;
    let opts = WaitOptions {
        interval: Duration::from_millis(20),
        timeout: Duration::from_millis(1),
        return_on_payment: true,
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
