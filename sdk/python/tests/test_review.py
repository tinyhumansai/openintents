import json
from pathlib import Path

import httpx

from openintents import AsyncOpenIntents, OpenIntents, OrderReview

EXAMPLES = Path(__file__).resolve().parents[3] / "spec" / "examples"


def example(name: str) -> dict:
    return json.loads((EXAMPLES / name).read_text())


def client(*bodies, calls):
    queue = list(bodies)

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request)
        status, body = queue.pop(0)
        return httpx.Response(status, json=body)

    http = httpx.Client(transport=httpx.MockTransport(handler))
    return OpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http, max_retries=0)


def test_create_sends_review_before_order_and_reads_the_review():
    calls = []
    oi = client((201, example("intent-requires-review.json")), calls=calls)
    got = oi.intents.create("snacks", pay_at_shop=True, review_before_order=True)
    assert json.loads(calls[0].content) == {
        "intent": "snacks",
        "pay_at_shop": True,
        "review_before_order": True,
    }
    assert got["status"] == "requires_input"
    assert got["review_before_order"] is True
    request = got["input_request"]
    assert request["kind"] == "review"
    review: OrderReview = request["review"]
    assert review == {
        "merchant": "blinkit.com",
        "items": [
            {"name": "Protein bar, sugar free", "quantity": 2, "amount": 10000},
            {"name": "Roasted chana", "quantity": 1, "amount": 4500},
        ],
        "fees": [
            {"label": "Delivery fee", "amount": 2500},
            {"label": "Cash on delivery fee", "amount": 1000},
        ],
        "total": 18000,
        "currency": "inr",
        "deliver_to": "Home, 12 MG Road, Bengaluru 560001",
        "eta": "12 minutes",
        "payment_method": "cash_on_delivery",
    }


def test_review_is_answered_with_done_a_change_or_a_decline():
    calls = []
    running = {**example("intent-requires-review.json"), "status": "running", "input_request": None}
    oi = client((200, running), (200, running), (200, example("intent-review-declined.json")), calls=calls)
    oi.intents.input("int_review_1", input="inp_review_1", done=True)
    oi.intents.input("int_review_1", input="inp_review_1", value="remove the chana")
    declined = oi.intents.input("int_review_1", input="inp_review_1", decline=True)
    assert [json.loads(c.content) for c in calls] == [
        {"input": "inp_review_1", "done": True},
        {"input": "inp_review_1", "value": "remove the chana"},
        {"input": "inp_review_1", "decline": True},
    ]
    assert {c.url.path for c in calls} == {"/v1/intents/int_review_1/input"}
    assert (declined["status"], declined["error"]["code"]) == ("failed", "order_declined")


def test_receipt_carries_the_approved_delivery_address():
    oi = client((200, example("intent-completed-reviewed.json")), calls=[])
    got = oi.intents.get("int_review_1")
    assert got["result"]["deliver_to"] == "Home, 12 MG Road, Bengaluru 560001"
    assert got["result"]["paid_with"] == "cash_on_delivery"


def test_a_question_of_an_unknown_kind_still_reads():
    body = example("intent-requires-review.json")
    body["input_request"]["kind"] = "something_new"
    del body["input_request"]["review"]
    oi = client((200, body), calls=[])
    got = oi.intents.get("int_review_1")
    assert got["input_request"]["kind"] == "something_new"
    assert "review" not in got["input_request"]


async def test_async_create_sends_review_before_order():
    calls = []

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request)
        return httpx.Response(201, json=example("intent-requires-review.json"))

    http = httpx.AsyncClient(transport=httpx.MockTransport(handler))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        got = await oi.intents.create("snacks", review_before_order=True)
    assert json.loads(calls[0].content) == {"intent": "snacks", "review_before_order": True}
    assert got["input_request"]["review"]["payment_method"] == "cash_on_delivery"
