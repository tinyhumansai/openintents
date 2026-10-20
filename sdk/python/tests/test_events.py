import json

import httpx
import pytest

from openintents import AsyncOpenIntents, OpenIntents, OpenIntentsError


def event(event_id, type="status", data=None):
    return {
        "id": event_id,
        "object": "event",
        "type": type,
        "created_at": "2026-10-01T08:30:00Z",
        "data": data if data is not None else {"status": "running"},
    }


def frame(e):
    """An SSE frame as the API sends it."""
    return f"id: {e['id']}\ndata: {json.dumps(e)}\n\n"


def sse(*chunks):
    """A text/event-stream reply whose body arrives in these pieces."""
    pieces = [c.encode() if isinstance(c, str) else c for c in chunks]
    return lambda: httpx.Response(200, content=iter(pieces), headers={"content-type": "text/event-stream"})


def status(code, body=None, headers=None):
    if body is None:
        return lambda: httpx.Response(code, headers=headers)
    return lambda: httpx.Response(code, json=body, headers=headers)


def replay(*replies, calls=None):
    """A MockTransport handler: each request gets the next reply, and the last one repeats."""
    queue = list(replies)

    def handler(request: httpx.Request) -> httpx.Response:
        if calls is not None:
            calls.append(request)
        return (queue.pop(0) if len(queue) > 1 else queue[0])()

    return handler


def client(*replies, calls=None, max_retries=0):
    http = httpx.Client(transport=httpx.MockTransport(replay(*replies, calls=calls)))
    return OpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http, max_retries=max_retries)


def test_list_events_pages():
    calls = []
    body = {"object": "list", "data": [event("evt_2")], "next_cursor": "evt_2"}
    oi = client(status(200, body), status(200, body), calls=calls)
    got = oi.intents.list_events("int_1", limit=1, cursor="evt_1")
    assert [e["id"] for e in got["data"]] == ["evt_2"]
    assert got["next_cursor"] == "evt_2"
    assert str(calls[0].url) == "https://api.test/v1/intents/int_1/events?limit=1&cursor=evt_1"
    assert calls[0].headers["accept"] == "application/json"
    oi.sessions.list_events("ses_1")
    assert str(calls[1].url) == "https://api.test/v1/sessions/ses_1/events"


def test_stream_skips_keep_alives_and_resumes_until_204():
    calls = []
    oi = client(
        sse(": keep-alive\n\n", frame(event("evt_1")) + frame(event("evt_2", "step", {"action": "open"}))),
        sse(frame(event("evt_3", "checkout", {"merchant": "Blue Bottle", "total": 550, "currency": "usd"}))),
        status(204),
        calls=calls,
    )
    got = list(oi.intents.stream_events("int_1"))
    assert [e["id"] for e in got] == ["evt_1", "evt_2", "evt_3"]
    assert got[1]["data"] == {"action": "open"}
    assert str(calls[0].url) == "https://api.test/v1/intents/int_1/events"
    assert calls[0].headers["accept"] == "text/event-stream"
    assert "last-event-id" not in calls[0].headers
    # The API closed the stream: it is resumed after the last event until it says the timeline is over.
    assert [c.headers["last-event-id"] for c in calls[1:]] == ["evt_2", "evt_3"]


def test_stream_parses_events_cut_anywhere():
    one = json.dumps(
        event("evt_1", "page", {"url": "https://shop.example/", "title": "Café"}), ensure_ascii=False
    )
    raw = f"id: evt_1\r\ndata: {one}\r\n\r\n".encode()
    cr, e = raw.index(b"\r"), raw.index("é".encode())
    two = f"id:evt_2\rdata:{json.dumps(event('evt_2'))}\r\r"
    oi = client(sse(raw[: cr + 1], raw[cr + 1 : e + 1], raw[e + 1 :], two), status(204))
    got = list(oi.sessions.stream_events("ses_1"))
    assert [ev["id"] for ev in got] == ["evt_1", "evt_2"]
    assert got[0]["data"]["title"] == "Café"


def test_stream_starts_after_last_event_id_and_retries_429():
    calls = []
    too_many = {
        "error": {"type": "rate_limit", "code": "too_many_streams", "message": "Too many open streams."}
    }
    oi = client(
        status(429, too_many, {"retry-after": "0.01"}),
        sse(frame(event("evt_10"))),
        status(204),
        calls=calls,
        max_retries=1,
    )
    assert [e["id"] for e in oi.sessions.stream_events("ses_1", last_event_id="evt_9")] == ["evt_10"]
    assert [c.headers["last-event-id"] for c in calls] == ["evt_9", "evt_9", "evt_10"]


def test_stream_ends_at_once_when_over():
    calls = []
    oi = client(status(204), calls=calls)
    assert list(oi.intents.stream_events("int_1", last_event_id="evt_9")) == []
    assert len(calls) == 1


def test_stream_raises_errors_not_worth_retrying():
    oi = client(status(404, {"error": {"type": "not_found", "message": "No such intent."}}), max_retries=2)
    with pytest.raises(OpenIntentsError) as exc:
        list(oi.intents.stream_events("int_x"))
    assert (exc.value.status, exc.value.type) == (404, "not_found")


def test_stream_gives_up_when_it_keeps_closing_empty():
    calls = []
    oi = client(sse(), calls=calls)
    with pytest.raises(OpenIntentsError) as exc:
        list(oi.intents.stream_events("int_1"))
    assert (exc.value.status, exc.value.type) == (0, "network")
    assert len(calls) == 1


async def test_async_events():
    calls = []

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request)
        if request.headers["accept"] == "application/json":
            return httpx.Response(200, json={"object": "list", "data": [event("evt_1")], "next_cursor": None})
        if "last-event-id" in request.headers:
            return httpx.Response(204)
        body = (": hi\n\n" + frame(event("evt_1")) + frame(event("evt_2"))).encode()
        return httpx.Response(200, content=body, headers={"content-type": "text/event-stream"})

    http = httpx.AsyncClient(transport=httpx.MockTransport(handler))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        assert [e["id"] async for e in oi.sessions.stream_events("ses_1")] == ["evt_1", "evt_2"]
        page = await oi.intents.list_events("int_1", limit=2)
        assert [e["id"] for e in page["data"]] == ["evt_1"]
    assert calls[0].url.path == "/v1/sessions/ses_1/events"
    assert calls[1].headers["last-event-id"] == "evt_2"
    assert str(calls[2].url) == "https://api.test/v1/intents/int_1/events?limit=2"
