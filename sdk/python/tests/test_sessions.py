import json

import httpx
import pytest

from openintents import AsyncOpenIntents, OpenIntents, OpenIntentsError


def replay(*replies, calls=None):
    """A MockTransport handler: each request gets the next reply, and the last one repeats."""
    queue = list(replies)

    def handler(request: httpx.Request) -> httpx.Response:
        if calls is not None:
            calls.append(request)
        reply = queue.pop(0) if len(queue) > 1 else queue[0]
        status, body = reply[0], reply[1]
        headers = reply[2] if len(reply) > 2 else None
        if isinstance(body, bytes):
            return httpx.Response(status, content=body, headers=headers)
        return httpx.Response(status, json=body, headers=headers)

    return handler


def client(*replies, calls=None):
    http = httpx.Client(transport=httpx.MockTransport(replay(*replies, calls=calls)))
    return OpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http, max_retries=0)


def profile(**over):
    return {
        "id": "prf_1",
        "object": "profile",
        "customer": "cus_1",
        "status": "ready",
        "lease": None,
        "size_bytes": None,
        "created_at": "2026-10-01T08:30:00Z",
        "last_used_at": None,
        **over,
    }


def session(**over):
    return {
        "id": "ses_1",
        "object": "session",
        "profile": "prf_1",
        "customer": "cus_1",
        "status": "queued",
        "test": True,
        "created_at": "2026-10-01T08:30:00Z",
        "expires_at": "2026-10-01T08:45:00Z",
        **over,
    }


def page(data, next_cursor=None):
    return {"object": "list", "data": data, "next_cursor": next_cursor}


def test_profiles_list_create_get_forget():
    calls = []
    oi = client(
        (200, page([profile()])),
        (201, profile()),
        (200, profile(customer=None)),
        (200, profile()),
        (202, profile(status="forgetting")),
        calls=calls,
    )
    assert oi.profiles.list(customer="cus_1", limit=5)["data"][0]["id"] == "prf_1"
    assert str(calls[0].url) == "https://api.test/v1/profiles?customer=cus_1&limit=5"

    oi.profiles.create(customer="cus_1")
    assert calls[1].method == "POST" and calls[1].url.path == "/v1/profiles"
    assert json.loads(calls[1].content) == {"customer": "cus_1"}
    # Making a profile is idempotent by itself: no key.
    assert "idempotency-key" not in calls[1].headers

    assert oi.profiles.create()["customer"] is None
    assert json.loads(calls[2].content) == {}

    oi.profiles.get("prf_1")
    assert calls[3].url.path == "/v1/profiles/prf_1"

    assert oi.profiles.forget("prf_1")["status"] == "forgetting"
    assert calls[4].method == "DELETE" and calls[4].url.path == "/v1/profiles/prf_1"


def test_profiles_list_all_follows_cursors():
    calls = []
    oi = client((200, page([profile(id="a")], "a")), (200, page([profile(id="b")])), calls=calls)
    assert [p["id"] for p in oi.profiles.list_all(customer="cus_1")] == ["a", "b"]
    assert calls[1].url.params["cursor"] == "a"
    assert calls[1].url.params["customer"] == "cus_1"


def test_sessions_create_list_get_release():
    calls = []
    oi = client(
        (201, session()),
        (200, page([session(status="open")], "ses_1")),
        (200, page([session(id="ses_0", status="open")])),
        (200, session(status="open", url="https://shop.example/login")),
        (200, session(status="closing")),
        calls=calls,
    )
    opened = oi.sessions.create(customer="cus_1", start_url="https://shop.example/login", max_minutes=10)
    assert opened["status"] == "queued"
    assert calls[0].method == "POST" and calls[0].url.path == "/v1/sessions"
    assert json.loads(calls[0].content) == {
        "customer": "cus_1",
        "start_url": "https://shop.example/login",
        "max_minutes": 10,
    }
    assert len(calls[0].headers["idempotency-key"]) == 36

    listed = oi.sessions.list_all(status="open", profile="prf_1", customer="cus_1", limit=1)
    assert [s["id"] for s in listed] == ["ses_1", "ses_0"]
    assert dict(calls[1].url.params) == {
        "status": "open",
        "profile": "prf_1",
        "customer": "cus_1",
        "limit": "1",
    }
    assert calls[2].url.params["cursor"] == "ses_1"

    assert oi.sessions.get("ses_1")["url"] == "https://shop.example/login"
    assert calls[3].url.path == "/v1/sessions/ses_1"

    assert oi.sessions.release("ses_1")["status"] == "closing"
    assert calls[4].method == "POST" and calls[4].url.path == "/v1/sessions/ses_1/release"
    assert calls[4].headers["idempotency-key"]


def test_run_command_done_pending_and_refused():
    result = {"id": "cmd_1", "object": "session_command", "created_at": "x"}
    refusal = {"code": "payment_not_allowed", "message": "A session never pays."}
    calls = []
    oi = client(
        (200, {**result, "type": "act", "status": "done", "url": "https://shop.example/login"}),
        (202, {**result, "type": "navigate", "status": "pending"}),
        (200, {**result, "type": "navigate", "status": "done", "completed_at": "y"}),
        (200, {**result, "type": "act", "status": "refused", "error": refusal}),
        calls=calls,
    )
    typed = oi.sessions.run_command(
        "ses_1", {"type": "act", "ref": "e12", "action": "type", "text": "ada@example.com"}
    )
    assert typed["status"] == "done"
    assert calls[0].url.path == "/v1/sessions/ses_1/commands"
    assert json.loads(calls[0].content) == {
        "type": "act",
        "ref": "e12",
        "action": "type",
        "text": "ada@example.com",
    }
    assert len(calls[0].headers["idempotency-key"]) == 36

    # A 202 still returns the command, "pending".
    pending = oi.sessions.run_command(
        "ses_1", {"type": "navigate", "url": "https://shop.example/cart"}, idempotency_key="nav-1"
    )
    assert pending["status"] == "pending"
    assert calls[1].headers["idempotency-key"] == "nav-1"

    assert oi.sessions.get_command("ses_1", "cmd_1")["status"] == "done"
    assert calls[2].url.path == "/v1/sessions/ses_1/commands/cmd_1"

    refused = oi.sessions.run_command("ses_1", {"type": "act", "ref": "e40", "action": "click"})
    assert refused["status"] == "refused"
    assert refused["error"]["code"] == "payment_not_allowed"


def test_screenshot_returns_jpeg_bytes():
    jpeg = b"\xff\xd8\xff\xe0"
    calls = []
    oi = client((200, jpeg, {"content-type": "image/jpeg"}), calls=calls)
    assert oi.sessions.screenshot("ses_1") == jpeg
    assert calls[0].url.path == "/v1/sessions/ses_1/screenshot"
    assert calls[0].headers["accept"] == "image/jpeg"


def test_errors_carry_the_lease_and_code():
    lease = {"holder": {"type": "intent", "id": "int_1"}, "expires_at": "2026-10-01T08:31:00Z"}
    busy = {"type": "conflict", "code": "profile_busy", "message": "Profile in use.", "lease": lease}
    not_open = {"type": "conflict", "code": "session_not_open", "message": "Not open."}
    oi = client((409, {"error": busy}), (409, {"error": not_open}))
    with pytest.raises(OpenIntentsError) as exc:
        oi.sessions.create()
    assert (exc.value.status, exc.value.code) == (409, "profile_busy")
    assert exc.value.lease == lease
    with pytest.raises(OpenIntentsError) as exc:
        oi.sessions.screenshot("ses_1")
    assert exc.value.code == "session_not_open"
    assert exc.value.lease is None


async def test_async_profiles_and_sessions():
    calls = []
    handler = replay(
        (201, profile()),
        (200, page([session()], "ses_1")),
        (200, page([session(id="ses_0")])),
        (202, {"id": "cmd_1", "object": "session_command", "type": "snapshot", "status": "pending"}),
        (200, b"\xff\xd8", {"content-type": "image/jpeg"}),
        (202, profile(status="forgetting")),
        calls=calls,
    )
    http = httpx.AsyncClient(transport=httpx.MockTransport(handler))
    async with AsyncOpenIntents("oi_test_x", base_url="https://api.test/v1", http_client=http) as oi:
        assert (await oi.profiles.create(customer="cus_1"))["id"] == "prf_1"
        assert [s["id"] async for s in oi.sessions.list_all(status="open")] == ["ses_1", "ses_0"]
        pending = await oi.sessions.run_command("ses_1", {"type": "snapshot"})
        assert pending["status"] == "pending"
        assert await oi.sessions.screenshot("ses_1") == b"\xff\xd8"
        assert (await oi.profiles.forget("prf_1"))["status"] == "forgetting"
    assert json.loads(calls[3].content) == {"type": "snapshot"}
    assert calls[3].headers["idempotency-key"]
    assert calls[5].method == "DELETE"
