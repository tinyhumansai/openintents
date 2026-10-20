"""Verify OpenIntents webhook signatures.

The `OpenIntents-Signature` header is `t=<unix seconds>,v1=<hex>`, where v1 is
HMAC-SHA256 of "{t}.{raw body}" keyed with your endpoint's `whsec_` secret.
While a secret is being rotated the header carries one v1 for each secret that
still signs ("t=...,v1=...,v1=..."), and either secret verifies it.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import time

from .errors import WebhookVerificationError
from .types import WebhookEvent

DEFAULT_TOLERANCE = 300


def _sign(secret: str, t: int, body: bytes) -> str:
    return hmac.new(secret.encode(), f"{t}.".encode() + body, hashlib.sha256).hexdigest()


def verify_webhook(
    raw_body: bytes | str,
    signature_header: str | None,
    secret: str,
    *,
    tolerance: int = DEFAULT_TOLERANCE,
    now: float | None = None,
) -> WebhookEvent:
    """Check the signature and return the parsed event.

    Pass the body exactly as received (bytes), before any JSON parsing.
    Raises WebhookVerificationError if it is missing, wrong or too old.
    """
    body = raw_body.encode() if isinstance(raw_body, str) else raw_body
    if not signature_header:
        raise WebhookVerificationError("Missing OpenIntents-Signature header")
    # After a secret rotation the header holds a v1 for each secret that still signs.
    pairs = [p.strip().split("=", 1) for p in signature_header.split(",") if "=" in p]
    v1s = [v.lower() for k, v in pairs if k == "v1"]
    try:
        t = int(next(v for k, v in pairs if k == "t"))
    except (StopIteration, ValueError) as exc:
        raise WebhookVerificationError("Malformed OpenIntents-Signature header") from exc
    if not v1s:
        raise WebhookVerificationError("Malformed OpenIntents-Signature header")
    if abs((now if now is not None else time.time()) - t) > tolerance:
        raise WebhookVerificationError("Signature timestamp outside tolerance")
    expected = _sign(secret, t, body)
    if not any([hmac.compare_digest(expected, v1) for v1 in v1s]):
        raise WebhookVerificationError("Signature mismatch")
    return json.loads(body)


def sign_webhook(raw_body: bytes | str, secret: str, t: int | None = None) -> str:
    """Build a signature header, for tests and local webhook replays."""
    body = raw_body.encode() if isinstance(raw_body, str) else raw_body
    ts = int(time.time()) if t is None else t
    return f"t={ts},v1={_sign(secret, ts, body)}"
