import json
import time

import pytest

from openintents import WebhookVerificationError, sign_webhook, verify_webhook

BODY = json.dumps(
    {"id": "evt_1", "type": "intent.completed", "created_at": "x", "data": {"id": "int_1"}}
).encode()
SECRET = "whsec_test"


def test_valid_signature():
    assert verify_webhook(BODY, sign_webhook(BODY, SECRET), SECRET)["type"] == "intent.completed"


def test_tampered_body():
    with pytest.raises(WebhookVerificationError, match="mismatch"):
        verify_webhook(BODY + b" ", sign_webhook(BODY, SECRET), SECRET)


def test_old_timestamp():
    with pytest.raises(WebhookVerificationError, match="tolerance"):
        verify_webhook(BODY, sign_webhook(BODY, SECRET, t=int(time.time()) - 3600), SECRET)


def test_missing_and_malformed():
    with pytest.raises(WebhookVerificationError, match="Missing"):
        verify_webhook(BODY, None, SECRET)
    with pytest.raises(WebhookVerificationError, match="Malformed"):
        verify_webhook(BODY, "v1=abc", SECRET)


def test_shared_test_vector():
    # The TypeScript and Rust suites assert the same value.
    assert sign_webhook(b'{"id":"evt_1"}', "whsec_x", t=1_700_000_000) == (
        "t=1700000000,v1=366e40c480fe3a9e6a03609dca079cea86d77ae82d5a56b5348208247317a8d1"
    )
