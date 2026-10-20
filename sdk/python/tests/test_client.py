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


def test_wait_returns_at_requires_login_with_the_link():
    sign_in = {"url": "https://openintents.io/signin/sgn_1", "shop": "amazon.com", "expires_at": "x"}
    oi = client((200, intent(status="running")), (200, intent(status="requires_login", sign_in=sign_in)))
    got = oi.intents.wait("int_1", interval=0)
    assert got["status"] == "requires_login"
    assert got["sign_in"]["url"] == "https://openintents.io/signin/sgn_1"


def test_create_sends_sign_in_link():
    calls = []
    oi = client((201, intent()), calls=calls)
    oi.intents.create("a lamp", sign_in_link=False)
    assert json.loads(calls[0].content) == {"intent": "a lamp", "sign_in_link": False}


def test_wait_keeps_polling_past_a_sign_in_when_asked():
    oi = client(
        (200, intent(status="requires_login")),
        (200, intent(status="queued")),
        (200, intent(status="completed")),
    )
    assert oi.intents.wait("int_1", interval=0, return_on_sign_in=False)["status"] == "completed"


INPUT_REQUEST = {
    "id": "inp_1",
    "kind": "code",
    "shop": "blinkit.com",
    "prompt": "Enter the code blinkit.com just sent to your phone.",
    "options": [],
    "channel": "sms",
    "length": 6,
    "url": "https://openintents.io/input/inp_1",
    "expires_at": "x",
    "attempts_left": 2,
}


def test_wait_returns_at_requires_input_with_what_to_ask():
    oi = client(
        (200, intent(status="running")),
        (200, intent(status="requires_input", input_request=INPUT_REQUEST)),
    )
    got = oi.intents.wait("int_1", interval=0)
    assert got["status"] == "requires_input"
    assert got["input_request"] == INPUT_REQUEST


def test_wait_returns_a_detail_with_its_options_answered_with_one():
    detail = {
        "id": "inp_2",
        "kind": "detail",
        "shop": "flipkart.com",
        "prompt": "Which size would you like?",
        "options": ["S", "M", "L"],
        "channel": None,
        "length": None,
        "url": "https://openintents.io/input/inp_2",
        "expires_at": "x",
        "attempts_left": 2,
    }
    calls = []
    oi = client(
        (200, intent(status="requires_input", input_request=detail)),
        (200, intent(status="running")),
        calls=calls,
    )
    request = oi.intents.wait("int_1", interval=0)["input_request"]
    assert request == detail
    assert request["kind"] == "detail" and request["options"] == ["S", "M", "L"]
    oi.intents.input("int_1", input=request["id"], value="M")
    assert json.loads(calls[1].content) == {"input": "inp_2", "value": "M"}


def test_wait_keeps_polling_past_an_input_request_when_asked():
    oi = client(
        (200, intent(status="requires_input")),
        (200, intent(status="running")),
        (200, intent(status="completed")),
    )
    assert oi.intents.wait("int_1", interval=0, return_on_input=False)["status"] == "completed"


def test_create_sends_sign_in_agent_and_hint():
    calls = []
    oi = client((201, intent()), calls=calls)
    oi.intents.create("a lamp", sign_in_agent=False, sign_in_hint={"phone": "+91 98765 43210"})
    assert json.loads(calls[0].content) == {
        "intent": "a lamp",
        "sign_in_agent": False,
        "sign_in_hint": {"phone": "+91 98765 43210"},
    }


PAYMENT_REQUEST = {
    "id": "inp_1",
    "kind": "payment",
    "shop": "blinkit.com",
    "prompt": "Pay 249.00 INR",
    "options": [],
    "choices": [],
    "fields": [],
    "expects": "text",
    "channel": None,
    "length": None,
    "url": "https://openintents.io/input/inp_1",
    "image_url": "https://openintents.io/input/inp_1/code.jpg",
    "expires_at": "2026-10-01T00:05:00Z",
    "attempts_left": 1,
}


def test_create_sends_pay_at_shop_and_reads_the_payment_code():
    calls = []
    oi = client((201, intent(status="requires_input", input_request=PAYMENT_REQUEST)), calls=calls)
    got = oi.intents.create("a lamp", pay_at_shop=True)
    assert json.loads(calls[0].content) == {"intent": "a lamp", "pay_at_shop": True}
    assert got["input_request"]["kind"] == "payment"
    assert got["input_request"]["image_url"] == "https://openintents.io/input/inp_1/code.jpg"


async def test_async_create_sends_pay_at_shop():
    calls = []
    handler = transport((201, intent(status="queued")), calls=calls)
    http = httpx.AsyncClient(transport=httpx.MockTransport(handler))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        await oi.intents.create("a lamp", pay_at_shop=True)
    assert json.loads(calls[0].content) == {"intent": "a lamp", "pay_at_shop": True}


def test_input_answers_with_a_value_a_decline_or_done():
    calls = []
    oi = client((200, intent(status="running")), calls=calls)
    assert oi.intents.input("int_1", input="inp_1", value="481516")["status"] == "running"
    assert calls[0].method == "POST" and calls[0].url.path == "/v1/intents/int_1/input"
    assert len(calls[0].headers["idempotency-key"]) == 36
    assert json.loads(calls[0].content) == {"input": "inp_1", "value": "481516"}
    oi.intents.input("int_1", input="inp_1", decline=True)
    assert json.loads(calls[1].content) == {"input": "inp_1", "decline": True}
    oi.intents.input("int_1", input="inp_2", done=True)
    assert json.loads(calls[2].content) == {"input": "inp_2", "done": True}


def test_input_too_late_raises_input_not_waiting():
    oi = client((409, {"error": {"type": "conflict", "code": "input_not_waiting", "message": "Expired."}}))
    with pytest.raises(OpenIntentsError) as exc:
        oi.intents.input("int_1", input="inp_1", value="481516")
    assert exc.value.status == 409
    assert exc.value.code == "input_not_waiting"


async def test_async_input_and_wait_at_requires_input():
    calls = []
    handler = transport(
        (200, intent(status="requires_input", input_request=INPUT_REQUEST)),
        (200, intent(status="running")),
        calls=calls,
    )
    http = httpx.AsyncClient(transport=httpx.MockTransport(handler))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        got = await oi.intents.wait("int_1", interval=0)
        assert got["input_request"]["id"] == "inp_1"
        answered = await oi.intents.input("int_1", input=got["input_request"]["id"], value="481516")
        assert answered["status"] == "running"
    assert calls[1].url.path == "/v1/intents/int_1/input"
    assert json.loads(calls[1].content) == {"input": "inp_1", "value": "481516"}


def test_balance():
    oi = client((200, {"object": "balance", "available": 24450, "currency": "usd"}))
    assert oi.balance.get()["available"] == 24450


def test_customer_balance_and_intents_for_a_customer():
    calls = []
    oi = client(
        (200, {"object": "balance", "available": 0, "held": 450, "currency": "usd", "customer": "cus_1"}),
        (201, intent(customer="cus_1")),
        (200, {"object": "list", "data": [], "next_cursor": None}),
        calls=calls,
    )
    assert oi.balance.get(customer="cus_1")["held"] == 450
    assert calls[0].url.params["customer"] == "cus_1"
    assert oi.intents.create("flat white", customer="cus_1")["customer"] == "cus_1"
    assert json.loads(calls[1].content) == {"intent": "flat white", "customer": "cus_1"}
    oi.intents.list(customer="cus_1")
    assert calls[2].url.params["customer"] == "cus_1"


def test_customers_create_update_list_get():
    customer = {"id": "cus_1", "object": "customer", "external_id": "u-42", "metadata": {}, "created_at": "x"}
    calls = []
    oi = client(
        (201, customer),
        (200, {**customer, "name": "Ada"}),
        (200, {"object": "list", "data": [customer], "next_cursor": None}),
        (200, customer),
        calls=calls,
    )
    oi.customers.create(external_id="u-42")
    assert calls[0].method == "POST" and str(calls[0].url) == "https://api.test/v1/customers"
    assert len(calls[0].headers["idempotency-key"]) == 36
    assert json.loads(calls[0].content) == {"external_id": "u-42"}
    assert oi.customers.update("cus_1", name="Ada", email="")["name"] == "Ada"
    assert calls[1].method == "PATCH" and calls[1].url.path == "/v1/customers/cus_1"
    assert json.loads(calls[1].content) == {"name": "Ada", "email": ""}
    assert [c["id"] for c in oi.customers.list_all(external_id="u-42")] == ["cus_1"]
    assert calls[2].url.params["external_id"] == "u-42"
    assert oi.customers.get("cus_1")["external_id"] == "u-42"


def test_topups_create_and_get():
    topup = {
        "id": "pay_1",
        "object": "topup",
        "amount": 5000,
        "currency": "usd",
        "customer": None,
        "status": "open",
        "url": "https://openintents.io/api/pay/pay_1",
        "expires_at": "x",
        "created_at": "x",
    }
    calls = []
    oi = client((201, topup), (200, {**topup, "status": "paid"}), calls=calls)
    assert oi.topups.create(5000)["status"] == "open"
    assert json.loads(calls[0].content) == {"amount": 5000}
    assert calls[0].headers["idempotency-key"]
    assert oi.topups.get("pay_1")["status"] == "paid"
    assert calls[1].url.path == "/v1/topups/pay_1"


async def test_async_customers_and_topups():
    calls = []
    handler = transport(
        (201, {"id": "cus_1", "object": "customer"}),
        (201, {"id": "pay_1", "object": "topup", "status": "open"}),
        calls=calls,
    )
    http = httpx.AsyncClient(transport=httpx.MockTransport(handler))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        assert (await oi.customers.create(name="Ada"))["id"] == "cus_1"
        assert (await oi.topups.create(5000, customer="cus_1"))["status"] == "open"
    assert json.loads(calls[1].content) == {"amount": 5000, "customer": "cus_1"}


async def test_async_client():
    http = httpx.AsyncClient(transport=httpx.MockTransport(transport((200, intent(status="completed")))))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        got = await oi.intents.wait("int_1", interval=0.001)
        assert got["status"] == "completed"
