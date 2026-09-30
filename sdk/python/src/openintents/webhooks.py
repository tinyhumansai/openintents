"""Verify OpenIntents webhook signatures.

The `OpenIntents-Signature` header is `t=<unix seconds>,v1=<hex>`, where v1 is
HMAC-SHA256 of "{t}.{raw body}" keyed with your endpoint's `whsec_` secret.
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
    parts = dict(p.strip().split("=", 1) for p in signature_header.split(",") if "=" in p)
    try:
        t = int(parts["t"])
        v1 = parts["v1"].lower()
    except (KeyError, ValueError) as exc:
        raise WebhookVerificationError("Malformed OpenIntents-Signature header") from exc
    if abs((now if now is not None else time.time()) - t) > tolerance:
        raise WebhookVerificationError("Signature timestamp outside tolerance")
    if not hmac.compare_digest(_sign(secret, t, body), v1):
        raise WebhookVerificationError("Signature mismatch")
    return json.loads(body)


def sign_webhook(raw_body: bytes | str, secret: str, t: int | None = None) -> str:
    """Build a signature header, for tests and local webhook replays."""
    body = raw_body.encode() if isinstance(raw_body, str) else raw_body
    ts = int(time.time()) if t is None else t
    return f"t={ts},v1={_sign(secret, ts, body)}"
