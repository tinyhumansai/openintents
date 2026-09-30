import json

import httpx
import pytest

from openintents import AsyncOpenIntents, OpenIntents, OpenIntentsError, WaitTimeoutError


def intent(**over):
    return {
        "id": "int_1",
        "object": "intent",
        "status": "queued",
        "intent": "flat white",
        "test": True,
        "created_at": "2026-10-01T08:30:00Z",
        **over,
    }


def transport(*responses, calls=None):
    queue = list(responses)

    def handler(request: httpx.Request) -> httpx.Response:
        if calls is not None:
            calls.append(request)
        status, body = queue.pop(0) if len(queue) > 1 else queue[0]
        return httpx.Response(status, json=body)

    return handler


def client(*responses, calls=None, **kw):
    http = httpx.Client(transport=httpx.MockTransport(transport(*responses, calls=calls)))
    return OpenIntents(
        "oi_test_x", base_url="https://api.test/v1", http_client=http, max_retries=kw.get("max_retries", 0)
    )


def test_requires_api_key(monkeypatch):
    monkeypatch.delenv("OPENINTENTS_API_KEY", raising=False)
    with pytest.raises(ValueError, match="API key"):
        OpenIntents()


def test_create_sends_auth_body_and_idempotency_key():
    calls = []
    oi = client((201, intent()), calls=calls)
    got = oi.intents.create("flat white", max_amount=1000)
    assert got["id"] == "int_1"
    req = calls[0]
    assert str(req.url) == "https://api.test/v1/intents"
    assert req.headers["authorization"] == "Bearer oi_test_x"
    assert len(req.headers["idempotency-key"]) == 36
    assert json.loads(req.content) == {"intent": "flat white", "max_amount": 1000}


def test_list_query_and_list_all_pagination():
    calls = []
    oi = client(
        (200, {"object": "list", "data": [intent(id="a")], "next_cursor": "a"}),
        (200, {"object": "list", "data": [intent(id="b")], "next_cursor": None}),
        calls=calls,
    )
    assert [i["id"] for i in oi.intents.list_all(status="completed")] == ["a", "b"]
    assert calls[0].url.params["status"] == "completed"
    assert calls[1].url.params["cursor"] == "a"


def test_errors_are_typed():
    oi = client(
        (400, {"error": {"type": "invalid_request", "code": "max_amount_too_low", "message": "too low"}})
    )
    with pytest.raises(OpenIntentsError) as exc:
        oi.intents.create("x")
    assert exc.value.status == 400
    assert exc.value.code == "max_amount_too_low"


def test_retries_5xx():
    calls = []
    oi = client((503, {}), (200, intent()), calls=calls, max_retries=1)
    oi._max_retries = 1
    assert oi.intents.get("int_1")["id"] == "int_1"
    assert len(calls) == 2


def test_wait_returns_payment_link():
    payment = {
        "reason": "insufficient_balance",
        "payment_url": "https://pay/p",
        "total": 550,
        "fee": 28,
        "currency": "usd",
        "expires_at": "x",
    }
    oi = client((200, intent(status="running")), (200, intent(status="requires_payment", payment=payment)))
    got = oi.intents.wait("int_1", interval=0.001)
    assert got["payment"]["payment_url"] == "https://pay/p"


def test_wait_past_payment_and_timeout():
    oi = client((200, intent(status="requires_payment")), (200, intent(status="completed")))
    assert oi.intents.wait("int_1", interval=0.001, return_on_payment=False)["status"] == "completed"
    oi2 = client((200, intent(status="running")))
    with pytest.raises(WaitTimeoutError):
        oi2.intents.wait("int_1", interval=0.01, timeout=0.001)


def test_balance():
    oi = client((200, {"object": "balance", "available": 24450, "currency": "usd"}))
    assert oi.balance.get()["available"] == 24450


async def test_async_client():
    http = httpx.AsyncClient(transport=httpx.MockTransport(transport((200, intent(status="completed")))))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        got = await oi.intents.wait("int_1", interval=0.001)
        assert got["status"] == "completed"
