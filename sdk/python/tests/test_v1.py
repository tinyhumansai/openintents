"""The v1 surface added since the first release: country, waiting, forms, secrets, sign-ins, webhooks."""

import json

import httpx
import pytest

from openintents import AsyncOpenIntents, OpenIntents, OpenIntentsError


def replay(*responses, calls):
    queue = list(responses)

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request)
        status, body = queue.pop(0) if len(queue) > 1 else queue[0]
        if status == 204:
            return httpx.Response(204)
        return httpx.Response(status, json=body)

    return handler


def client(*responses, calls, **kw):
    http = httpx.Client(transport=httpx.MockTransport(replay(*responses, calls=calls)))
    return OpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http, max_retries=0, **kw)


def body(call):
    return json.loads(call.content)


def test_create_sends_country_webhook_secrets_and_the_rest():
    calls = []
    oi = client((201, {"id": "int_1"}), calls=calls)
    oi.intents.create(
        "flat white",
        country="gb",
        end_user_ip="203.0.113.9",
        webhook_url="https://h.example/hook",
        input_ttl_seconds=600,
        secrets=["sec_1"],
        shops=["amazon.co.uk"],
    )
    assert body(calls[0]) == {
        "intent": "flat white",
        "country": "gb",
        "end_user_ip": "203.0.113.9",
        "webhook_url": "https://h.example/hook",
        "input_ttl_seconds": 600,
        "secrets": ["sec_1"],
        "shops": ["amazon.co.uk"],
    }


def test_an_empty_secrets_list_is_sent_so_none_are_offered():
    calls = []
    client((201, {"id": "int_1"}), calls=calls).intents.create("x", secrets=[])
    assert body(calls[0])["secrets"] == []


def test_wait_holds_the_request_with_prefer_and_stretches_the_timeout():
    calls = []
    oi = client((200, {"id": "int_1", "status": "completed"}), (201, {"id": "int_1"}), calls=calls)
    oi.intents.get("int_1", wait=30)
    assert calls[0].headers["prefer"] == "wait=30"
    assert calls[0].extensions["timeout"]["read"] >= 30
    oi.intents.create("flat white", wait=20)
    assert calls[1].headers["prefer"] == "wait=20"
    assert calls[1].headers["idempotency-key"]


def test_no_prefer_header_without_wait():
    calls = []
    client((200, {"id": "int_1"}), calls=calls).intents.get("int_1")
    assert "prefer" not in calls[0].headers


def test_input_posts_form_values():
    calls = []
    client((200, {"id": "int_1"}), calls=calls).intents.input(
        "int_1", input="inp_1", values={"seat_row": "12", "seat": "a"}
    )
    assert body(calls[0]) == {"input": "inp_1", "values": {"seat_row": "12", "seat": "a"}}


def test_a_429_carries_retry_after_and_the_rate_limit():
    calls = []
    http = httpx.Client(
        transport=httpx.MockTransport(
            lambda r: httpx.Response(
                429,
                json={"error": {"type": "rate_limit", "code": "rate_limit_exceeded", "message": "slow"}},
                headers={
                    "Retry-After": "7",
                    "RateLimit-Limit": "300",
                    "RateLimit-Remaining": "0",
                    "RateLimit-Reset": "7",
                },
            )
        )
    )
    oi = OpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http, max_retries=0)
    with pytest.raises(OpenIntentsError) as exc:
        oi.intents.get("int_1")
    assert exc.value.code == "rate_limit_exceeded"
    assert exc.value.retry_after == 7
    assert exc.value.rate_limit == {"limit": 300, "remaining": 0, "reset": 7}
    del calls


def test_errors_without_the_headers_have_neither():
    calls = []
    oi = client((404, {"error": {"type": "not_found", "message": "no"}}), calls=calls)
    with pytest.raises(OpenIntentsError) as exc:
        oi.intents.get("x")
    assert exc.value.retry_after is None
    assert exc.value.rate_limit is None


def test_customers_limits_portal_sessions_and_secret_requests():
    calls = []
    oi = client(
        (201, {"id": "cus_1"}),
        (201, {"object": "portal_session", "url": "https://x/p"}),
        (201, {"object": "portal_session", "url": "https://x/p"}),
        (201, {"object": "secret_request", "id": "sr_1", "url": "https://x/s"}),
        (200, {"object": "secret_request", "id": "sr_1", "state": "done", "secrets": ["sec_1"]}),
        calls=calls,
    )
    oi.customers.create(limits={"per_intent": 5000, "monthly": None})
    assert body(calls[0]) == {"limits": {"per_intent": 5000, "monthly": None}}
    got = oi.customers.create_portal_session("cus_1", ttl_seconds=3600, country="de")
    assert got["url"] == "https://x/p"
    assert str(calls[1].url) == "https://api.test/v1/customers/cus_1/portal_sessions"
    assert body(calls[1]) == {"ttl_seconds": 3600, "country": "de"}
    assert "idempotency-key" not in calls[1].headers
    oi.customers.create_portal_session("cus_1")
    assert body(calls[2]) == {}
    made = oi.customers.create_secret_request("cus_1", kinds=["address", "contact"])
    assert made["url"] == "https://x/s"
    assert str(calls[3].url) == "https://api.test/v1/customers/cus_1/secret_requests"
    assert body(calls[3]) == {"kinds": ["address", "contact"]}
    assert oi.secret_requests.get("sr_1")["secrets"] == ["sec_1"]
    assert str(calls[4].url) == "https://api.test/v1/secret_requests/sr_1"


def test_secrets_resource():
    calls = []
    oi = client(
        (201, {"id": "sec_1", "object": "secret"}),
        (200, {"object": "list", "data": [], "next_cursor": None}),
        (200, {"id": "sec_1"}),
        (200, {"id": "sec_1", "version": 2}),
        (200, {"object": "list", "data": [], "next_cursor": None}),
        (204, None),
        calls=calls,
    )
    oi.secrets.create(
        kind="address", label="Home", fields={"line1": "1 St"}, shops=["amazon.com"], default=True
    )
    assert str(calls[0].url) == "https://api.test/v1/secrets"
    assert body(calls[0]) == {
        "kind": "address",
        "label": "Home",
        "fields": {"line1": "1 St"},
        "shops": ["amazon.com"],
        "default": True,
    }
    assert calls[0].headers["idempotency-key"]
    oi.secrets.list(customer="cus_1", kind="loyalty")
    assert str(calls[1].url) == "https://api.test/v1/secrets?customer=cus_1&kind=loyalty"
    oi.secrets.get("sec_1")
    oi.secrets.replace("sec_1", kind="contact", label="Me", fields={"email": "a@b.co"})
    assert calls[3].method == "PUT"
    assert calls[3].headers["idempotency-key"]
    oi.secrets.uses("sec_1", limit=5)
    assert str(calls[4].url) == "https://api.test/v1/secrets/sec_1/uses?limit=5"
    assert oi.secrets.delete("sec_1") is None
    assert calls[5].method == "DELETE"


def test_sign_ins_and_live_links():
    calls = []
    oi = client(
        (201, {"id": "sgn_1", "url": "https://x/signin/sgn_1"}),
        (200, {"id": "sgn_1", "status": "done"}),
        (201, {"object": "live_link", "url": "https://x/live"}),
        calls=calls,
    )
    made = oi.sign_ins.create(
        shop_url="https://www.amazon.com/", customer="cus_1", return_url="https://me.example"
    )
    assert made["url"] == "https://x/signin/sgn_1"
    assert str(calls[0].url) == "https://api.test/v1/sign_ins"
    assert body(calls[0]) == {
        "shop_url": "https://www.amazon.com/",
        "customer": "cus_1",
        "return_url": "https://me.example",
    }
    assert calls[0].headers["idempotency-key"]
    assert oi.sign_ins.get("sgn_1")["status"] == "done"
    link = oi.sessions.create_live_link("ses_1")
    assert link["url"] == "https://x/live"
    assert str(calls[2].url) == "https://api.test/v1/sessions/ses_1/live_links"
    assert calls[2].content == b""


def test_session_create_takes_country_and_webhook_url():
    calls = []
    client((201, {"id": "ses_1"}), calls=calls).sessions.create(
        country="fr", end_user_ip="203.0.113.9", webhook_url="https://h.example/s"
    )
    assert body(calls[0]) == {
        "country": "fr",
        "end_user_ip": "203.0.113.9",
        "webhook_url": "https://h.example/s",
    }


def test_webhook_endpoints_resource():
    calls = []
    oi = client(
        (201, {"id": "we_1", "secret": "whsec_a"}),
        (200, {"object": "list", "data": [{"id": "a"}], "next_cursor": "a"}),
        (200, {"object": "list", "data": [{"id": "b"}], "next_cursor": None}),
        (200, {"id": "we_1"}),
        (200, {"id": "we_1", "disabled": True}),
        (204, None),
        (200, {"id": "we_1", "secret": "whsec_b"}),
        (201, {"id": "whd_1", "status": "pending"}),
        (200, {"object": "list", "data": [], "next_cursor": None}),
        (201, {"id": "whd_2", "redelivery_of": "whd_1"}),
        calls=calls,
    )
    made = oi.webhook_endpoints.create(url="https://h.example/hook", events=["sign_in.completed"])
    assert made["secret"] == "whsec_a"
    assert body(calls[0]) == {"url": "https://h.example/hook", "events": ["sign_in.completed"]}
    assert calls[0].headers["idempotency-key"]
    assert [e["id"] for e in oi.webhook_endpoints.list_all(limit=1)] == ["a", "b"]
    assert "cursor=a" in str(calls[2].url)
    oi.webhook_endpoints.get("we_1")
    oi.webhook_endpoints.update("we_1", disabled=True)
    assert calls[4].method == "PATCH"
    assert body(calls[4]) == {"disabled": True}
    assert oi.webhook_endpoints.delete("we_1") is None
    assert calls[5].method == "DELETE"
    assert oi.webhook_endpoints.rotate_secret("we_1")["secret"] == "whsec_b"
    assert str(calls[6].url) == "https://api.test/v1/webhook_endpoints/we_1/rotate_secret"
    assert oi.webhook_endpoints.test("we_1")["status"] == "pending"
    assert str(calls[7].url) == "https://api.test/v1/webhook_endpoints/we_1/test"
    oi.webhook_endpoints.list_deliveries("we_1", limit=10)
    assert str(calls[8].url) == "https://api.test/v1/webhook_endpoints/we_1/deliveries?limit=10"
    assert oi.webhook_endpoints.redeliver("we_1", "whd_1")["redelivery_of"] == "whd_1"
    assert str(calls[9].url) == "https://api.test/v1/webhook_endpoints/we_1/deliveries/whd_1/redeliver"


async def test_async_client_has_the_same_surface():
    calls = []
    handler = replay(
        (201, {"id": "int_1"}),
        (201, {"id": "sec_1"}),
        (204, None),
        (201, {"id": "sgn_1"}),
        (201, {"object": "live_link"}),
        (201, {"object": "portal_session"}),
        (200, {"id": "we_1"}),
        (200, {"id": "we_1", "secret": "whsec_b"}),
        (201, {"id": "whd_1"}),
        (200, {"object": "list", "data": [{"id": "a"}], "next_cursor": None}),
        calls=calls,
    )
    http = httpx.AsyncClient(transport=httpx.MockTransport(handler))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        await oi.intents.create("x", country="gb", wait=5)
        assert calls[0].headers["prefer"] == "wait=5"
        assert body(calls[0])["country"] == "gb"
        await oi.secrets.create(kind="contact", label="Me", fields={"email": "a@b.co"})
        assert await oi.secrets.delete("sec_1") is None
        await oi.sign_ins.create(shop_url="https://www.amazon.com/")
        await oi.sessions.create_live_link("ses_1")
        await oi.customers.create_portal_session("cus_1", ttl_seconds=60)
        await oi.webhook_endpoints.get("we_1")
        assert (await oi.webhook_endpoints.rotate_secret("we_1"))["secret"] == "whsec_b"
        await oi.webhook_endpoints.test("we_1")
        assert [e["id"] async for e in oi.webhook_endpoints.list_all()] == ["a"]
