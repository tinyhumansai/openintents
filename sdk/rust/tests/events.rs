use std::future::poll_fn;
use std::pin::Pin;

use futures_core::Stream;
use openintents::{Client, Error, Event, EventStream, ListEvents};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn client(server: &MockServer, max_retries: u32) -> Client {
    Client::builder("oi_test_x")
        .base_url(server.uri())
        .max_retries(max_retries)
        .build()
}

fn event(id: &str, kind: &str, data: serde_json::Value) -> serde_json::Value {
    json!({ "id": id, "object": "event", "type": kind, "created_at": "2026-10-01T08:30:00Z", "data": data })
}

/// An SSE frame as the API sends it.
fn frame(event: &serde_json::Value) -> String {
    format!("id: {}\ndata: {}\n\n", event["id"].as_str().unwrap(), event)
}

fn sse(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body, "text/event-stream")
}

fn fresh(req: &Request) -> bool {
    !req.headers.contains_key("last-event-id")
}

async fn ids(mut events: EventStream) -> Vec<String> {
    let mut ids = Vec::new();
    while let Some(event) = events.next().await {
        ids.push(event.unwrap().id);
    }
    ids
}

#[test]
fn event_streams_can_move_between_tasks() {
    fn assert_send_unpin<T: Send + Unpin + Stream>() {}
    assert_send_unpin::<EventStream>();
}

#[tokio::test]
async fn lists_a_page_of_events() {
    let server = MockServer::start().await;
    let page = json!({ "object": "list", "data": [event("evt_2", "status", json!({ "status": "running" }))],
                       "next_cursor": "evt_2" });
    Mock::given(method("GET"))
        .and(path("/intents/int_1/events"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "evt_1"))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sessions/ses_1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page))
        .expect(1)
        .mount(&server)
        .await;
    let oi = client(&server, 0);
    let params = ListEvents {
        limit: Some(1),
        cursor: Some("evt_1".into()),
    };
    let got = oi.list_intent_events("int_1", &params).await.unwrap();
    assert_eq!(
        (got.data[0].kind.as_str(), got.next_cursor.as_deref()),
        ("status", Some("evt_2"))
    );
    oi.list_session_events("ses_1", &ListEvents::default()).await.unwrap();
}

#[tokio::test]
async fn streams_skips_keep_alives_and_resumes_until_204() {
    let server = MockServer::start().await;
    let first = format!(
        ": keep-alive\n\n{}{}",
        frame(&event("evt_1", "status", json!({ "status": "running" }))),
        frame(&event("evt_2", "step", json!({ "action": "open" })))
    );
    Mock::given(method("GET"))
        .and(path("/intents/int_1/events"))
        .and(header("accept", "text/event-stream"))
        .and(fresh)
        .respond_with(sse(first))
        .expect(1)
        .mount(&server)
        .await;
    // The API closed the stream: it is resumed after the last event, until it says the timeline is over.
    let checkout = event(
        "evt_3",
        "checkout",
        json!({ "merchant": "Blue Bottle", "total": 550, "currency": "usd" }),
    );
    Mock::given(method("GET"))
        .and(header("last-event-id", "evt_2"))
        .respond_with(sse(frame(&checkout)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(header("last-event-id", "evt_3"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let mut events = client(&server, 0).stream_intent_events("int_1", None);
    let mut got: Vec<Event> = Vec::new();
    while let Some(event) = events.next().await {
        got.push(event.unwrap());
    }
    let ids: Vec<&str> = got.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["evt_1", "evt_2", "evt_3"]);
    assert_eq!(got[1].data, json!({ "action": "open" }));
    assert_eq!(got[2].kind, "checkout");
    // Once over, it stays over.
    assert!(events.next().await.is_none());
}

#[tokio::test]
async fn starts_after_last_event_id_and_retries_a_429() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sessions/ses_1/events"))
        .and(header("last-event-id", "evt_9"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0.01").set_body_json(
            json!({ "error": { "type": "rate_limit", "code": "too_many_streams", "message": "Too many open streams." } }),
        ))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(header("last-event-id", "evt_9"))
        .respond_with(sse(frame(&event(
            "evt_10",
            "page",
            json!({ "url": "https://shop.example/" }),
        ))))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(header("last-event-id", "evt_10"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    // Read it as a futures `Stream`.
    let mut events = client(&server, 1).stream_session_events("ses_1", Some("evt_9"));
    let first = poll_fn(|cx| Pin::new(&mut events).poll_next(cx)).await;
    assert_eq!(first.unwrap().unwrap().id, "evt_10");
    assert!(poll_fn(|cx| Pin::new(&mut events).poll_next(cx)).await.is_none());
}

#[tokio::test]
async fn ends_at_once_when_the_timeline_is_over() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let events = client(&server, 1).stream_intent_events("int_1", Some("evt_9"));
    assert!(ids(events).await.is_empty());
}

#[tokio::test]
async fn ends_on_errors_not_worth_retrying() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({ "error": { "type": "not_found", "message": "No such intent." } })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut events = client(&server, 2).stream_intent_events("int_x", None);
    match events.next().await {
        Some(Err(Error::Api { status, kind, .. })) => assert_eq!((status, kind.as_str()), (404, "not_found")),
        other => panic!("expected a 404, got {other:?}"),
    }
    assert!(events.next().await.is_none());
}

#[tokio::test]
async fn gives_up_on_a_stream_that_keeps_closing_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(sse(String::new()))
        .expect(1)
        .mount(&server)
        .await;
    let mut events = client(&server, 0).stream_intent_events("int_1", None);
    assert!(matches!(events.next().await, Some(Err(Error::StreamClosed))));
}
