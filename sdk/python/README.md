# openintents (Python)

Python SDK for [OpenIntents](https://openintents.io): let your agents buy
anything. Send an intent in plain language and it gets bought end to end in a
real browser, paid, and resolved to a receipt. Inference, browser,
compute and bandwidth are free.

> Preview: the API is still being built, so it may change before launch.

```bash
pip install openintents
```

## Usage

```python
from openintents import OpenIntents

oi = OpenIntents()  # reads OPENINTENTS_API_KEY

intent = oi.intents.create(
    "flat white, oat milk, pickup 8:45 at the Blue Bottle on 5th",
    max_amount=1000,  # cents: never spend more than $10.00
)

intent = oi.intents.wait(intent["id"])  # returns when final, or when it needs payment

if intent["status"] == "requires_payment":
    print("Pay here:", intent["payment"]["payment_url"])
elif intent["status"] == "completed":
    receipt = intent["result"]
    print(receipt["merchant"], receipt["total"], receipt["confirmation"])
```

Async:

```python
from openintents import AsyncOpenIntents

async with AsyncOpenIntents() as oi:
    intent = await oi.intents.create("book an Uber to SFO at 7am", max_amount=6000)
    intent = await oi.intents.wait(intent["id"])
```

Also: `oi.intents.get(id)`, `oi.intents.list(status=..., limit=...)`,
`oi.intents.list_all()`, `oi.intents.cancel(id)` and `oi.balance.get()`.
Errors raise `OpenIntentsError` (`status`, `type`, `code`, `param`); 429s,
5xx and network errors are retried with backoff.

## Webhooks

```python
from openintents import verify_webhook, WebhookVerificationError

event = verify_webhook(request_body_bytes, request.headers.get("OpenIntents-Signature"), "whsec_...")
```

## Development

```bash
uv run --extra dev pytest
uv run --extra dev ruff check src tests
```

Docs: <https://openintents.io/docs>. License: GPL-3.0-or-later.
