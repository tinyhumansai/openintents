# openintents (Python)

Python SDK for [OpenIntents](https://openintents.io), the agentic API to buy
anything. Send an intent in plain language; the API does the agentic work in a
real browser, gets it paid, and resolves to a receipt. Inference, browser,
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
    print("Send the payer:", intent["payment"]["payment_url"])  # a step in their wallet
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

Also: `oi.intents.get(id)`, `oi.intents.list(status=..., customer=..., limit=...)`,
`oi.intents.list_all()`, `oi.intents.cancel(id)`, `oi.balance.get(customer=...)`,
`oi.customers.create/get/update/list/list_all` and `oi.topups.create/get`.
Pass `customer=` to `intents.create` to buy for one of your customers.
Errors raise `OpenIntentsError` (`status`, `type`, `code`, `param`, and
`lease` with `profile_busy`); 429s, 5xx and network errors are retried with
backoff. Every create (but a profile's), `cancel`, `release` and
`run_command` sends an `Idempotency-Key` automatically.

## Live events

Follow an intent as it runs. The stream resumes by itself after a dropped
connection, and ends once the intent is over:

```python
for event in oi.intents.stream_events(intent["id"]):
    print(event["type"], event["data"])  # status, step, checkout
```

`oi.intents.list_events(id, limit=..., cursor=...)` pages the same timeline.
On `AsyncOpenIntents`, `stream_events` is an async iterator: `async for`.

## Browser sessions

A session opens a payer's browser profile without an intent: to sign them in
to a shop once, so later intents find them signed in. A session never pays
or places an order.

```python
session = oi.sessions.create(customer="cus_4b81d0", start_url="https://shop.example/login")
for event in oi.sessions.stream_events(session["id"]):
    if event["type"] == "status" and event["data"]["status"] == "open":
        break
page = oi.sessions.run_command(session["id"], {"type": "snapshot"})  # page["snapshot"] has the refs
oi.sessions.run_command(session["id"], {"type": "act", "ref": "e12", "action": "type", "text": "ada@example.com"})
jpeg = oi.sessions.screenshot(session["id"])  # bytes
oi.sessions.release(session["id"])  # the profile keeps the sign-in
```

Also `oi.sessions.list/list_all/get/get_command/list_events` and
`oi.profiles.list/list_all/create/get/forget`.

## Country, waiting and forms

`country` (ISO 3166-1 alpha-2, lowercase) or `end_user_ip` on
`intents.create`, `sessions.create`, `sign_ins.create` and
`customers.create_portal_session` picks the country the browser goes out
from; the object says which in `country` and `country_source`. Pass
`wait=30` to `intents.create` or `intents.get` to have the API hold the
request up to 50 seconds until the intent finishes or pauses.

A question in `input_request` can offer `choices` (answer with one's `id`) or
ask a form of `fields` (answer with `values={"name": "value"}`); `expects`
says what a plain answer looks like. A `sensitive` question (an identity
document) is answered only on `input_request["url"]`. With
`pay_at_shop=True` on create (live keys), the payer pays the shop themselves:
a `payment` question shows the shop's payment code at
`input_request["image_url"]`, answered with `done=True` once they paid.

## Secrets, sign-ins and live links

```python
card = oi.secrets.create(kind="card", label="Work Visa", fields={"number": "4242424242424242"}, shops=["airfrance.com"])
oi.intents.create("book the 7am flight", secrets=[card["id"]], shops=["airfrance.com"])

ask = oi.customers.create_secret_request("cus_4b81d0", kinds=["identity_document"])  # send ask["url"]
sign_in = oi.sign_ins.create(shop_url="https://www.amazon.com/", customer="cus_4b81d0")  # send sign_in["url"]
portal = oi.customers.create_portal_session("cus_4b81d0", ttl_seconds=1800)  # send portal["url"]
live = oi.sessions.create_live_link(session["id"])  # live["url"] drives the session, never to pay
```

Secrets are write-only: they come back as a label, a mask and public fields,
never their values (`oi.secrets.list/get/replace/delete/uses`).

## Webhook endpoints

```python
endpoint = oi.webhook_endpoints.create(url="https://example.com/hooks", events=["intent.completed", "sign_in.completed"])
# endpoint["secret"] is shown here only. Also: list/list_all/get/update/delete.
oi.webhook_endpoints.rotate_secret(endpoint["id"])  # the old secret signs for 24 more hours
oi.webhook_endpoints.test(endpoint["id"])  # sends webhook_endpoint.test to it alone
oi.webhook_endpoints.list_deliveries(endpoint["id"])
```

A 429 `OpenIntentsError` carries `retry_after` and `rate_limit` (`limit`,
`remaining`, `reset`, from the `RateLimit-*` headers).

## Webhooks

```python
from openintents import verify_webhook, WebhookVerificationError

event = verify_webhook(request_body_bytes, request.headers.get("OpenIntents-Signature"), "whsec_...")
```

Session events (`session.opened`, `session.closed`, `session.failed`) carry a
session in `data`: check `event["type"]` first.

## Development

```bash
uv run --extra dev pytest
uv run --extra dev ruff check src tests
```

Docs: <https://openintents.io/docs>. License: GPL-3.0-or-later.
