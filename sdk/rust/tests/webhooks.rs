use openintents::webhooks::{sign_webhook, verify_webhook, verify_webhook_at};
use openintents::{IntentStatus, SessionStatus, WebhookData};

const BODY: &[u8] = br#"{"id":"evt_1","type":"intent.completed","created_at":"x","data":{"id":"int_1","status":"completed","intent":"x","created_at":"x"}}"#;
const SECRET: &str = "whsec_test";

#[test]
fn accepts_a_valid_signature() {
    let header = sign_webhook(BODY, SECRET, None);
    let event = verify_webhook(BODY, Some(&header), SECRET).unwrap();
    assert_eq!(event.kind, "intent.completed");
    assert_eq!(event.intent().unwrap().status, IntentStatus::Completed);
    assert!(event.session().is_none());
}

#[test]
fn session_events_carry_a_session_and_unknown_types_parse() {
    let body = br#"{"id":"evt_2","type":"session.closed","created_at":"x","data":{"id":"ses_1","object":"session","profile":"prf_1","status":"closed","close_reason":"idle","created_at":"x","expires_at":"y"}}"#;
    let event = verify_webhook(body, Some(&sign_webhook(body, SECRET, None)), SECRET).unwrap();
    let session = event.session().unwrap();
    assert_eq!(session.status, SessionStatus::Closed);
    assert_eq!(session.close_reason.as_deref(), Some("idle"));

    let body = br#"{"id":"evt_3","type":"profile.forgotten","created_at":"x","data":{"id":"prf_1"}}"#;
    let event = verify_webhook(body, Some(&sign_webhook(body, SECRET, None)), SECRET).unwrap();
    assert!(matches!(event.data, WebhookData::Other(_)));
}

#[test]
fn rejects_tampering_wrong_secret_and_old_timestamps() {
    let header = sign_webhook(BODY, SECRET, None);
    assert!(verify_webhook(b"{}", Some(&header), SECRET).is_err());
    assert!(verify_webhook(BODY, Some(&sign_webhook(BODY, "whsec_other", None)), SECRET).is_err());
    let old = sign_webhook(BODY, SECRET, Some(1_000));
    assert!(verify_webhook_at(BODY, Some(&old), SECRET, 300, 10_000).is_err());
    assert!(verify_webhook(BODY, None, SECRET).is_err());
}

/// Shared test vector: the TypeScript and Python suites assert the same value.
#[test]
fn matches_the_shared_test_vector() {
    assert_eq!(
        sign_webhook(br#"{"id":"evt_1"}"#, "whsec_x", Some(1_700_000_000)),
        "t=1700000000,v1=366e40c480fe3a9e6a03609dca079cea86d77ae82d5a56b5348208247317a8d1"
    );
}

#[test]
fn a_rotation_header_with_a_v1_for_each_secret_matches_either_secret() {
    let new = sign_webhook(BODY, "whsec_new", Some(1_000));
    let old = sign_webhook(BODY, "whsec_old", Some(1_000));
    let header = format!("{new},{}", old.split_once(',').unwrap().1);
    assert_eq!(header.matches("v1=").count(), 2);
    for secret in ["whsec_new", "whsec_old"] {
        assert!(verify_webhook_at(BODY, Some(&header), secret, 300, 1_000).is_ok());
    }
    assert!(verify_webhook_at(BODY, Some(&header), "whsec_other", 300, 1_000).is_err());
}

#[test]
fn verifies_an_independently_generated_sha256_signature() {
    let header = "t=1700000000,v1=31803bafa1c9d0226517b39774c4b887a22916d9e3ea5ef08bcbd166647af6f3";
    let event = verify_webhook_at(BODY, Some(header), SECRET, 300, 1_700_000_000).unwrap();
    assert_eq!(event.kind, "intent.completed");
    assert!(verify_webhook_at(BODY, Some(header), "whsec_other", 300, 1_700_000_000).is_err());
}
