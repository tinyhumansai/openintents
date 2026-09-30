use openintents::webhooks::{sign_webhook, verify_webhook, verify_webhook_at};

const BODY: &[u8] = br#"{"id":"evt_1","type":"intent.completed","created_at":"x","data":{"id":"int_1","status":"completed","intent":"x","created_at":"x"}}"#;
const SECRET: &str = "whsec_test";

#[test]
fn accepts_a_valid_signature() {
    let header = sign_webhook(BODY, SECRET, None);
    let event = verify_webhook(BODY, Some(&header), SECRET).unwrap();
    assert_eq!(event.kind, "intent.completed");
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
