"""OpenIntents: let your agents buy anything.

from openintents import OpenIntents

oi = OpenIntents()  # reads OPENINTENTS_API_KEY
intent = oi.intents.create("flat white, oat milk, pickup 8:45", max_amount=1000)
intent = oi.intents.wait(intent["id"])
if intent["status"] == "requires_payment":
    print("Pay here:", intent["payment"]["payment_url"])
"""

from ._client import DEFAULT_BASE_URL, AsyncOpenIntents, OpenIntents
from ._version import __version__
from .errors import OpenIntentsError, WaitTimeoutError, WebhookVerificationError
from .types import (
    FINAL_STATUSES,
    Balance,
    Intent,
    IntentList,
    IntentStatus,
    PaymentRequest,
    Receipt,
    WebhookEvent,
)
from .webhooks import sign_webhook, verify_webhook

__all__ = [
    "DEFAULT_BASE_URL",
    "FINAL_STATUSES",
    "AsyncOpenIntents",
    "Balance",
    "Intent",
    "IntentList",
    "IntentStatus",
    "OpenIntents",
    "OpenIntentsError",
    "PaymentRequest",
    "Receipt",
    "WaitTimeoutError",
    "WebhookEvent",
    "WebhookVerificationError",
    "__version__",
    "sign_webhook",
    "verify_webhook",
]
