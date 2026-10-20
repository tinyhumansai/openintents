"""Sync and async clients for the OpenIntents agentic API."""

from __future__ import annotations

import asyncio
import codecs
import json
import os
import random
import re
import time
import uuid
from collections.abc import AsyncIterator, Iterator, Mapping, Sequence
from typing import Any

import httpx

from ._v1 import (
    AsyncSecretRequests,
    AsyncSecrets,
    AsyncSignIns,
    AsyncWebhookEndpoints,
    SecretRequests,
    Secrets,
    SignIns,
    WebhookEndpoints,
)
from ._version import __version__
from .errors import OpenIntentsError, WaitTimeoutError
from .types import (
    FINAL_STATUSES,
    Customer,
    CustomerList,
    Event,
    EventList,
    Intent,
    IntentList,
    IntentStatus,
    LimitsParams,
    LiveLink,
    PortalSession,
    Profile,
    ProfileList,
    RateLimit,
    SecretKind,
    SecretRequestCreated,
    Session,
    SessionCommand,
    SessionCommandResult,
    SessionList,
    SessionStatus,
    SignInHint,
)

DEFAULT_BASE_URL = "https://api.openintents.io/v1"

#: A stream with no event and no keep-alive for this long (or the client
#: timeout, if longer) is dropped and resumed.
STREAM_IDLE_TIMEOUT = 60.0


def _resolve(api_key: str | None, base_url: str | None) -> tuple[str, str]:
    key = api_key or os.environ.get("OPENINTENTS_API_KEY")
    if not key:
        raise ValueError("Missing API key: pass api_key= or set OPENINTENTS_API_KEY.")
    url = (base_url or os.environ.get("OPENINTENTS_BASE_URL") or DEFAULT_BASE_URL).rstrip("/")
    return key, url


def _headers(
    api_key: str, idempotency_key: str | None, accept: str = "application/json", wait: int | None = None
) -> dict[str, str]:
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Accept": accept,
        "User-Agent": f"openintents-python/{__version__}",
    }
    if idempotency_key:
        headers["Idempotency-Key"] = idempotency_key
    if wait is not None:
        headers["Prefer"] = f"wait={wait}"
    return headers


def _hold_timeout(timeout: float, wait: int | None) -> httpx.Timeout | None:
    """A held request (Prefer: wait=N) outlasts the client's timeout: the API holds it up to 50 seconds."""
    if wait is None:
        return None
    return httpx.Timeout(max(timeout, min(wait, 50) + 10.0))


def _stream_headers(api_key: str, last_event_id: str | None) -> dict[str, str]:
    headers = _headers(api_key, None, "text/event-stream")
    if last_event_id is not None:
        headers["Last-Event-ID"] = last_event_id
    return headers


def _stream_timeout(timeout: float) -> httpx.Timeout:
    # The read timeout runs between chunks, so keep-alives hold a quiet stream open.
    return httpx.Timeout(timeout, read=max(timeout, STREAM_IDLE_TIMEOUT))


def _backoff(attempt: int, retry_after: str | None) -> float:
    try:
        if retry_after is not None and float(retry_after) > 0:
            return float(retry_after)
    except ValueError:
        pass
    return min(8.0, 0.5 * 2**attempt) * random.uniform(0.75, 1.25)


def _retryable(status: int) -> bool:
    return status == 429 or status >= 500


def _header_int(res: httpx.Response, name: str) -> int | None:
    try:
        return int(float(res.headers[name]))
    except (KeyError, ValueError):
        return None


def _rate_limit(res: httpx.Response) -> RateLimit | None:
    limit, remaining, reset = (_header_int(res, f"ratelimit-{k}") for k in ("limit", "remaining", "reset"))
    if limit is None or remaining is None or reset is None:
        return None
    return {"limit": limit, "remaining": remaining, "reset": reset}


def _error(res: httpx.Response) -> OpenIntentsError:
    try:
        err = res.json().get("error", {}) or {}
    except ValueError:
        err = {}
    return OpenIntentsError(
        err.get("message") or f"HTTP {res.status_code}",
        status=res.status_code,
        type=err.get("type") or ("api_error" if res.status_code >= 500 else "invalid_request"),
        code=err.get("code"),
        param=err.get("param"),
        lease=err.get("lease"),
        request_id=res.headers.get("request-id"),
        retry_after=_header_int(res, "retry-after"),
        rate_limit=_rate_limit(res),
    )


def _network_error(cause: object) -> OpenIntentsError:
    return OpenIntentsError(f"Network error: {cause}", status=0, type="network")


class _SseParser:
    """Splits a Server-Sent Events stream into messages.

    By the rules of the HTML standard: lines end in CRLF, LF or CR, comment
    lines (":", the stream's keep-alives) and unknown fields are skipped, and a
    blank line ends a message. Only messages that carry data are returned, each
    as (the last id the stream had sent by then, data).
    """

    _EOL = re.compile(r"\r\n|\r|\n")

    def __init__(self) -> None:
        self._decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")
        self._id: str | None = None
        self._buffer = ""
        self._data: list[str] = []

    def feed(self, chunk: bytes | None) -> list[tuple[str | None, str]]:
        """The messages a chunk of the body completes; None means the body ended."""
        if chunk is None:
            # A message without its closing blank line is dropped, as the standard says.
            out = self._push(self._decoder.decode(b"", final=True))
            rest, self._buffer = self._buffer, ""
            if rest.endswith("\r"):
                message = self._line(rest[:-1])
                if message:
                    out.append(message)
            return out
        return self._push(self._decoder.decode(chunk))

    def _push(self, text: str) -> list[tuple[str | None, str]]:
        self._buffer += text
        out: list[tuple[str | None, str]] = []
        start = 0
        for m in self._EOL.finditer(self._buffer):
            # A CR at the very end may be the first half of a CRLF cut in two.
            if m.group() == "\r" and m.end() == len(self._buffer):
                break
            message = self._line(self._buffer[start : m.start()])
            if message:
                out.append(message)
            start = m.end()
        self._buffer = self._buffer[start:]
        return out

    def _line(self, line: str) -> tuple[str | None, str] | None:
        if not line:
            if not self._data:
                return None
            data, self._data = "\n".join(self._data), []
            return self._id, data
        if line.startswith(":"):
            return None
        field, _, value = line.partition(":")
        if value.startswith(" "):
            value = value[1:]
        if field == "data":
            self._data.append(value)
        elif field == "id" and "\0" not in value:
            self._id = value
        return None


def _create_body(
    intent: str,
    max_amount: int | None,
    currency: str | None,
    context: Mapping[str, Any] | None,
    require_payment_link: bool | None,
    metadata: Mapping[str, str] | None,
    customer: str | None,
    sign_in_link: bool | None = None,
    sign_in_agent: bool | None = None,
    sign_in_hint: SignInHint | None = None,
    country: str | None = None,
    end_user_ip: str | None = None,
    webhook_url: str | None = None,
    input_ttl_seconds: int | None = None,
    secrets: Sequence[str] | None = None,
    shops: Sequence[str] | None = None,
) -> dict[str, Any]:
    body: dict[str, Any] = {"intent": intent}
    for k, v in (
        ("max_amount", max_amount),
        ("currency", currency),
        ("context", dict(context) if context is not None else None),
        ("require_payment_link", require_payment_link),
        ("metadata", dict(metadata) if metadata is not None else None),
        ("customer", customer),
        ("sign_in_link", sign_in_link),
        ("sign_in_agent", sign_in_agent),
        ("sign_in_hint", dict(sign_in_hint) if sign_in_hint is not None else None),
        ("country", country),
        ("end_user_ip", end_user_ip),
        ("webhook_url", webhook_url),
        ("input_ttl_seconds", input_ttl_seconds),
        ("secrets", list(secrets) if secrets is not None else None),
        ("shops", list(shops) if shops is not None else None),
    ):
        if v is not None:
            body[k] = v
    return body


def _answer_body(
    input_id: str,
    value: str | None,
    decline: bool,
    done: bool,
    values: Mapping[str, str] | None = None,
) -> dict[str, Any]:
    # Only the answer given: the API wants exactly one of value, values, decline and done.
    body: dict[str, Any] = {"input": input_id}
    if value is not None:
        body["value"] = value
    if values is not None:
        body["values"] = dict(values)
    if decline:
        body["decline"] = True
    if done:
        body["done"] = True
    return body


def _fields(**fields: Any) -> dict[str, Any]:
    """The fields given: None means left out (an empty string clears one)."""
    return {k: (dict(v) if isinstance(v, Mapping) else v) for k, v in fields.items() if v is not None}


def _settled(intent: Intent, return_on_payment: bool, return_on_sign_in: bool, return_on_input: bool) -> bool:
    status = intent.get("status")
    return (
        status in FINAL_STATUSES
        or (return_on_payment and status == "requires_payment")
        or (return_on_sign_in and status == "requires_login")
        or (return_on_input and status == "requires_input")
    )


class OpenIntents:
    """Synchronous client.

    >>> oi = OpenIntents()  # reads OPENINTENTS_API_KEY
    >>> intent = oi.intents.create("flat white, oat milk", max_amount=1000)
    >>> oi.intents.wait(intent["id"])
    """

    def __init__(
        self,
        api_key: str | None = None,
        *,
        base_url: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 2,
        http_client: httpx.Client | None = None,
    ) -> None:
        self._api_key, self._base_url = _resolve(api_key, base_url)
        self._max_retries = max_retries
        self._timeout = timeout
        self._http = http_client or httpx.Client(timeout=timeout)
        self.intents = Intents(self)
        self.customers = Customers(self)
        self.profiles = Profiles(self)
        self.sessions = Sessions(self)
        self.secrets = Secrets(self)
        self.secret_requests = SecretRequests(self)
        self.sign_ins = SignIns(self)
        self.webhook_endpoints = WebhookEndpoints(self)

    def close(self) -> None:
        self._http.close()

    def __enter__(self) -> OpenIntents:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def _request(
        self,
        method: str,
        path: str,
        *,
        json: Any = None,
        params: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        wait: int | None = None,
    ) -> Any:
        return self._send(
            method, path, json=json, params=params, idempotency_key=idempotency_key, wait=wait
        ).json()

    def _send(
        self,
        method: str,
        path: str,
        *,
        json: Any = None,
        params: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        accept: str = "application/json",
        wait: int | None = None,
    ) -> httpx.Response:
        query = {k: v for k, v in (params or {}).items() if v is not None}
        headers = _headers(self._api_key, idempotency_key, accept, wait)
        hold = _hold_timeout(self._timeout, wait)
        extra: dict[str, Any] = {} if hold is None else {"timeout": hold}
        attempt = 0
        while True:
            try:
                res = self._http.request(
                    method, self._base_url + path, json=json, params=query, headers=headers, **extra
                )
            except httpx.TransportError as exc:
                if attempt < self._max_retries:
                    time.sleep(_backoff(attempt, None))
                    attempt += 1
                    continue
                raise _network_error(exc) from exc
            if res.is_success:
                return res
            if _retryable(res.status_code) and attempt < self._max_retries:
                time.sleep(_backoff(attempt, res.headers.get("retry-after")))
                attempt += 1
                continue
            raise _error(res)

    def _stream(self, path: str, last_event_id: str | None) -> Iterator[Event]:
        """Follow a timeline over Server-Sent Events: past events, then new ones.

        A stream that closes or drops is resumed after the last event received
        (Last-Event-ID) until the API answers 204: the timeline is over and
        every event was sent.
        """
        last_id = last_event_id
        failures = 0
        while True:
            retry_after: str | None = None
            try:
                with self._http.stream(
                    "GET",
                    self._base_url + path,
                    headers=_stream_headers(self._api_key, last_id),
                    timeout=_stream_timeout(self._timeout),
                ) as res:
                    if res.status_code == 204:
                        return
                    if not res.is_success:
                        res.read()
                        if not _retryable(res.status_code):
                            raise _error(res)
                        error = _error(res)
                        retry_after = res.headers.get("retry-after")
                    else:
                        parser = _SseParser()
                        sent_anything = False
                        for chunk in res.iter_bytes():
                            if chunk:
                                # Anything arriving, a keep-alive too, shows the stream works.
                                sent_anything, failures = True, 0
                            for event_id, data in parser.feed(chunk):
                                event = json.loads(data)
                                last_id = event_id or event.get("id") or last_id
                                yield event
                        for event_id, data in parser.feed(None):
                            event = json.loads(data)
                            last_id = event_id or event.get("id") or last_id
                            yield event
                        if sent_anything:
                            # The API closes a stream now and then: resume at once.
                            continue
                        # One that closes before sending a byte counts as a failure, so it cannot spin.
                        error = _network_error("the event stream closed empty")
            except httpx.TransportError as exc:
                error = _network_error(exc)
            if failures >= self._max_retries:
                raise error
            time.sleep(_backoff(failures, retry_after))
            failures += 1


class Intents:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def create(
        self,
        intent: str,
        *,
        max_amount: int | None = None,
        currency: str | None = None,
        context: Mapping[str, Any] | None = None,
        require_payment_link: bool | None = None,
        metadata: Mapping[str, str] | None = None,
        customer: str | None = None,
        sign_in_link: bool | None = None,
        sign_in_agent: bool | None = None,
        sign_in_hint: SignInHint | None = None,
        country: str | None = None,
        end_user_ip: str | None = None,
        webhook_url: str | None = None,
        input_ttl_seconds: int | None = None,
        secrets: Sequence[str] | None = None,
        shops: Sequence[str] | None = None,
        wait: int | None = None,
        idempotency_key: str | None = None,
    ) -> Intent:
        """Start an intent. Returns immediately, usually with status "queued".

        With wait=N (seconds) the request is held until the intent has
        something to tell you (see get), so a short purchase can be one call.
        country= is where the payer's browser goes out from (ISO 3166-1
        alpha-2, lowercase), or end_user_ip= lets it be inferred; the intent
        says which in "country" and "country_source".
        """
        body = _create_body(
            intent,
            max_amount,
            currency,
            context,
            require_payment_link,
            metadata,
            customer,
            sign_in_link,
            sign_in_agent,
            sign_in_hint,
            country,
            end_user_ip,
            webhook_url,
            input_ttl_seconds,
            secrets,
            shops,
        )
        return self._c._request(
            "POST", "/intents", json=body, idempotency_key=idempotency_key or str(uuid.uuid4()), wait=wait
        )

    def get(self, intent_id: str, *, wait: int | None = None) -> Intent:
        """The intent. With wait=N (seconds, 50 at most) the request is held until the intent is
        neither "queued" nor "running", or N seconds pass: a hold that runs out is no error."""
        return self._c._request("GET", f"/intents/{intent_id}", wait=wait)

    def list(
        self,
        *,
        status: IntentStatus | None = None,
        customer: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> IntentList:
        return self._c._request(
            "GET",
            "/intents",
            params={"status": status, "customer": customer, "limit": limit, "cursor": cursor},
        )

    def list_all(
        self, *, status: IntentStatus | None = None, customer: str | None = None, limit: int | None = None
    ) -> Iterator[Intent]:
        """Iterate every intent matching the filters, following next_cursor."""
        cursor: str | None = None
        while True:
            page = self.list(status=status, customer=customer, limit=limit, cursor=cursor)
            yield from page.get("data", [])
            cursor = page.get("next_cursor")
            if not cursor:
                return

    def cancel(self, intent_id: str, *, idempotency_key: str | None = None) -> Intent:
        """Cancel an intent that hasn't paid yet. Cancelled intents are free."""
        return self._c._request(
            "POST", f"/intents/{intent_id}/cancel", idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    def input(
        self,
        intent_id: str,
        *,
        input: str,
        value: str | None = None,
        values: Mapping[str, str] | None = None,
        decline: bool = False,
        done: bool = False,
        idempotency_key: str | None = None,
    ) -> Intent:
        """Answer the intent's input_request while it waits in "requires_input".

        Pass its id as input=, and one of value= (the phone number, email,
        code, or a "detail"'s answer: one of its options, or the id of one of
        its choices), values= (a form's answers by field name, for a request
        with fields), decline=True (the payer would rather not) or done=True (only for an
        "approval", once they approved it). Returns the intent, usually
        "running" again.
        A request already answered or expired raises a 409 input_not_waiting.

        >>> request = intent["input_request"]
        >>> oi.intents.input(intent["id"], input=request["id"], value="481516")
        """
        return self._c._request(
            "POST",
            f"/intents/{intent_id}/input",
            json=_answer_body(input, value, decline, done, values),
            idempotency_key=idempotency_key or str(uuid.uuid4()),
        )

    def list_events(
        self, intent_id: str, *, limit: int | None = None, cursor: str | None = None
    ) -> EventList:
        """A page of the intent's timeline, oldest first: status changes, the agent's steps, its checkout."""
        return self._c._request(
            "GET", f"/intents/{intent_id}/events", params={"limit": limit, "cursor": cursor}
        )

    def stream_events(self, intent_id: str, *, last_event_id: str | None = None) -> Iterator[Event]:
        """Follow the intent's timeline live: past events first, then new ones as they happen.

        Resumes by itself after a dropped connection, and ends once the intent is
        over and its last event was sent. Pass last_event_id to start after it.

        >>> for event in oi.intents.stream_events(intent["id"]):
        ...     print(event["type"], event["data"])
        """
        return self._c._stream(f"/intents/{intent_id}/events", last_event_id)

    def wait(
        self,
        intent_id: str,
        *,
        timeout: float = 300.0,
        interval: float = 2.0,
        return_on_payment: bool = True,
        return_on_sign_in: bool = True,
        return_on_input: bool = True,
    ) -> Intent:
        """Poll until the intent is final, or (by default) waits on a person.

        On "requires_payment", hand intent["payment"]["payment_url"] to a person.
        On "requires_input", ask the payer intent["input_request"]["prompt"],
        offering its options if it has any, and answer with input(). On
        "requires_login", send intent["sign_in"]["url"] to the payer only.
        """
        deadline = time.monotonic() + timeout
        while True:
            intent = self.get(intent_id)
            if _settled(intent, return_on_payment, return_on_sign_in, return_on_input):
                return intent
            if time.monotonic() + interval > deadline:
                raise WaitTimeoutError(intent_id, timeout)
            time.sleep(interval)


class Customers:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def create(
        self,
        *,
        external_id: str | None = None,
        name: str | None = None,
        email: str | None = None,
        metadata: Mapping[str, str] | None = None,
        limits: LimitsParams | None = None,
        idempotency_key: str | None = None,
    ) -> Customer:
        """Someone you buy for, with their own wallet and browser profile."""
        body = _fields(external_id=external_id, name=name, email=email, metadata=metadata, limits=limits)
        return self._c._request(
            "POST", "/customers", json=body, idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    def get(self, customer_id: str) -> Customer:
        return self._c._request("GET", f"/customers/{customer_id}")

    def update(
        self,
        customer_id: str,
        *,
        external_id: str | None = None,
        name: str | None = None,
        email: str | None = None,
        metadata: Mapping[str, str] | None = None,
        limits: LimitsParams | None = None,
    ) -> Customer:
        """Fields left out stay; "" clears external_id, name or email; metadata replaces the whole map."""
        body = _fields(external_id=external_id, name=name, email=email, metadata=metadata, limits=limits)
        return self._c._request("PATCH", f"/customers/{customer_id}", json=body)

    def create_portal_session(
        self,
        customer_id: str,
        *,
        return_url: str | None = None,
        country: str | None = None,
        end_user_ip: str | None = None,
        ttl_seconds: int | None = None,
    ) -> PortalSession:
        """A link to a page where the customer sees and manages their own browser.

        They see the shops they are signed in to, can sign in to another, look
        around, and have their profile forgotten. Send url to them only; it works
        once. Until portal sessions are open to your key this raises a 503
        "portal_unavailable".
        """
        body = _fields(
            return_url=return_url, country=country, end_user_ip=end_user_ip, ttl_seconds=ttl_seconds
        )
        return self._c._request("POST", f"/customers/{customer_id}/portal_sessions", json=body)

    def create_secret_request(
        self,
        customer_id: str,
        *,
        kinds: Sequence[SecretKind],
        return_url: str | None = None,
        ttl_seconds: int | None = None,
    ) -> SecretRequestCreated:
        """Ask a customer to enter their own secrets (an address, a contact, an identity
        document) on a page of their own, so they never pass through you.

        Send them url; read the result with secret_requests.get or the
        "secret_request.completed" webhook.
        """
        body = _fields(kinds=list(kinds), return_url=return_url, ttl_seconds=ttl_seconds)
        return self._c._request("POST", f"/customers/{customer_id}/secret_requests", json=body)

    def list(
        self, *, external_id: str | None = None, limit: int | None = None, cursor: str | None = None
    ) -> CustomerList:
        """Newest first."""
        return self._c._request(
            "GET", "/customers", params={"external_id": external_id, "limit": limit, "cursor": cursor}
        )

    def list_all(self, *, external_id: str | None = None, limit: int | None = None) -> Iterator[Customer]:
        """Iterate every customer matching the filters, following next_cursor."""
        cursor: str | None = None
        while True:
            page = self.list(external_id=external_id, limit=limit, cursor=cursor)
            yield from page.get("data", [])
            cursor = page.get("next_cursor")
            if not cursor:
                return


class Profiles:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def list(
        self, *, customer: str | None = None, limit: int | None = None, cursor: str | None = None
    ) -> ProfileList:
        """Your account's profile and your customers' that exist so far, newest first."""
        return self._c._request(
            "GET", "/profiles", params={"customer": customer, "limit": limit, "cursor": cursor}
        )

    def list_all(self, *, customer: str | None = None, limit: int | None = None) -> Iterator[Profile]:
        """Iterate every profile matching the filters, following next_cursor."""
        cursor: str | None = None
        while True:
            page = self.list(customer=customer, limit=limit, cursor=cursor)
            yield from page.get("data", [])
            cursor = page.get("next_cursor")
            if not cursor:
                return

    def create(self, *, customer: str | None = None) -> Profile:
        """Make the profile of your account, or of customer, ahead of its first intent or session.

        Optional: an intent makes the profile it needs. Making one that exists
        returns it unchanged, so no Idempotency-Key is needed.
        """
        return self._c._request("POST", "/profiles", json=_fields(customer=customer))

    def get(self, profile_id: str) -> Profile:
        return self._c._request("GET", f"/profiles/{profile_id}")

    def forget(self, profile_id: str) -> Profile:
        """Erase everything the profile kept: logins, cookies, saved data.

        Returns it "forgetting", then it turns "forgotten". Forgetting twice is fine.
        """
        return self._c._request("DELETE", f"/profiles/{profile_id}")


class Sessions:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def create(
        self,
        *,
        profile: str | None = None,
        customer: str | None = None,
        start_url: str | None = None,
        max_minutes: int | None = None,
        country: str | None = None,
        end_user_ip: str | None = None,
        webhook_url: str | None = None,
        idempotency_key: str | None = None,
    ) -> Session:
        """Open a browser on a profile without an intent: by profile or customer, else your account's.

        A profile in use raises a 409 "profile_busy" whose lease says what holds it.
        """
        body = _fields(
            profile=profile,
            customer=customer,
            start_url=start_url,
            max_minutes=max_minutes,
            country=country,
            end_user_ip=end_user_ip,
            webhook_url=webhook_url,
        )
        return self._c._request(
            "POST", "/sessions", json=body, idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    def get(self, session_id: str) -> Session:
        return self._c._request("GET", f"/sessions/{session_id}")

    def list(
        self,
        *,
        status: SessionStatus | None = None,
        profile: str | None = None,
        customer: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> SessionList:
        """Newest first."""
        return self._c._request(
            "GET",
            "/sessions",
            params={
                "status": status,
                "profile": profile,
                "customer": customer,
                "limit": limit,
                "cursor": cursor,
            },
        )

    def list_all(
        self,
        *,
        status: SessionStatus | None = None,
        profile: str | None = None,
        customer: str | None = None,
        limit: int | None = None,
    ) -> Iterator[Session]:
        """Iterate every session matching the filters, following next_cursor."""
        cursor: str | None = None
        while True:
            page = self.list(status=status, profile=profile, customer=customer, limit=limit, cursor=cursor)
            yield from page.get("data", [])
            cursor = page.get("next_cursor")
            if not cursor:
                return

    def release(self, session_id: str, *, idempotency_key: str | None = None) -> Session:
        """Close the browser, keeping what the session changed in the profile.

        A session that already ended is returned unchanged.
        """
        return self._c._request(
            "POST", f"/sessions/{session_id}/release", idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    def run_command(
        self, session_id: str, command: SessionCommand, *, idempotency_key: str | None = None
    ) -> SessionCommandResult:
        """Run one command on the session's page.

        It waits up to 20 seconds; a command still running then comes back
        "pending", to follow with get_command. A refused or failed one says why
        in "error".

        >>> oi.sessions.run_command(session["id"], {"type": "snapshot"})
        >>> oi.sessions.run_command(session["id"], {"type": "act", "ref": "e12", "action": "click"})
        """
        return self._c._request(
            "POST",
            f"/sessions/{session_id}/commands",
            json=dict(command),
            idempotency_key=idempotency_key or str(uuid.uuid4()),
        )

    def get_command(self, session_id: str, command_id: str) -> SessionCommandResult:
        return self._c._request("GET", f"/sessions/{session_id}/commands/{command_id}")

    def create_live_link(self, session_id: str) -> LiveLink:
        """A link to the session's live view. Whoever holds url sees the browser and drives it,
        though never to pay or place an order. Each call makes a new link, valid for 10 minutes.
        The session must be open."""
        return self._c._request("POST", f"/sessions/{session_id}/live_links")

    def screenshot(self, session_id: str) -> bytes:
        """A screenshot of the session's page, taken now: JPEG bytes."""
        return self._c._send("GET", f"/sessions/{session_id}/screenshot", accept="image/jpeg").content

    def list_events(
        self, session_id: str, *, limit: int | None = None, cursor: str | None = None
    ) -> EventList:
        """A page of the session's timeline, oldest first: status changes, pages and commands."""
        return self._c._request(
            "GET", f"/sessions/{session_id}/events", params={"limit": limit, "cursor": cursor}
        )

    def stream_events(self, session_id: str, *, last_event_id: str | None = None) -> Iterator[Event]:
        """Follow the session's timeline live: past events first, then new ones as they happen.

        Resumes by itself after a dropped connection, and ends once the session
        is over and its last event was sent. Pass last_event_id to start after it.
        """
        return self._c._stream(f"/sessions/{session_id}/events", last_event_id)


class AsyncOpenIntents:
    """Asynchronous client, same surface as OpenIntents with awaitable methods."""

    def __init__(
        self,
        api_key: str | None = None,
        *,
        base_url: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 2,
        http_client: httpx.AsyncClient | None = None,
    ) -> None:
        self._api_key, self._base_url = _resolve(api_key, base_url)
        self._max_retries = max_retries
        self._timeout = timeout
        self._http = http_client or httpx.AsyncClient(timeout=timeout)
        self.intents = AsyncIntents(self)
        self.customers = AsyncCustomers(self)
        self.profiles = AsyncProfiles(self)
        self.sessions = AsyncSessions(self)
        self.secrets = AsyncSecrets(self)
        self.secret_requests = AsyncSecretRequests(self)
        self.sign_ins = AsyncSignIns(self)
        self.webhook_endpoints = AsyncWebhookEndpoints(self)

    async def aclose(self) -> None:
        await self._http.aclose()

    async def __aenter__(self) -> AsyncOpenIntents:
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self.aclose()

    async def _request(
        self,
        method: str,
        path: str,
        *,
        json: Any = None,
        params: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        wait: int | None = None,
    ) -> Any:
        res = await self._send(
            method, path, json=json, params=params, idempotency_key=idempotency_key, wait=wait
        )
        return res.json()

    async def _send(
        self,
        method: str,
        path: str,
        *,
        json: Any = None,
        params: Mapping[str, Any] | None = None,
        idempotency_key: str | None = None,
        accept: str = "application/json",
        wait: int | None = None,
    ) -> httpx.Response:
        query = {k: v for k, v in (params or {}).items() if v is not None}
        headers = _headers(self._api_key, idempotency_key, accept, wait)
        hold = _hold_timeout(self._timeout, wait)
        extra: dict[str, Any] = {} if hold is None else {"timeout": hold}
        attempt = 0
        while True:
            try:
                res = await self._http.request(
                    method, self._base_url + path, json=json, params=query, headers=headers, **extra
                )
            except httpx.TransportError as exc:
                if attempt < self._max_retries:
                    await asyncio.sleep(_backoff(attempt, None))
                    attempt += 1
                    continue
                raise _network_error(exc) from exc
            if res.is_success:
                return res
            if _retryable(res.status_code) and attempt < self._max_retries:
                await asyncio.sleep(_backoff(attempt, res.headers.get("retry-after")))
                attempt += 1
                continue
            raise _error(res)

    async def _stream(self, path: str, last_event_id: str | None) -> AsyncIterator[Event]:
        """The async twin of OpenIntents._stream."""
        last_id = last_event_id
        failures = 0
        while True:
            retry_after: str | None = None
            try:
                async with self._http.stream(
                    "GET",
                    self._base_url + path,
                    headers=_stream_headers(self._api_key, last_id),
                    timeout=_stream_timeout(self._timeout),
                ) as res:
                    if res.status_code == 204:
                        return
                    if not res.is_success:
                        await res.aread()
                        if not _retryable(res.status_code):
                            raise _error(res)
                        error = _error(res)
                        retry_after = res.headers.get("retry-after")
                    else:
                        parser = _SseParser()
                        sent_anything = False
                        async for chunk in res.aiter_bytes():
                            if chunk:
                                sent_anything, failures = True, 0
                            for event_id, data in parser.feed(chunk):
                                event = json.loads(data)
                                last_id = event_id or event.get("id") or last_id
                                yield event
                        for event_id, data in parser.feed(None):
                            event = json.loads(data)
                            last_id = event_id or event.get("id") or last_id
                            yield event
                        if sent_anything:
                            continue
                        error = _network_error("the event stream closed empty")
            except httpx.TransportError as exc:
                error = _network_error(exc)
            if failures >= self._max_retries:
                raise error
            await asyncio.sleep(_backoff(failures, retry_after))
            failures += 1


class AsyncIntents:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def create(
        self,
        intent: str,
        *,
        max_amount: int | None = None,
        currency: str | None = None,
        context: Mapping[str, Any] | None = None,
        require_payment_link: bool | None = None,
        metadata: Mapping[str, str] | None = None,
        customer: str | None = None,
        sign_in_link: bool | None = None,
        sign_in_agent: bool | None = None,
        sign_in_hint: SignInHint | None = None,
        country: str | None = None,
        end_user_ip: str | None = None,
        webhook_url: str | None = None,
        input_ttl_seconds: int | None = None,
        secrets: Sequence[str] | None = None,
        shops: Sequence[str] | None = None,
        wait: int | None = None,
        idempotency_key: str | None = None,
    ) -> Intent:
        body = _create_body(
            intent,
            max_amount,
            currency,
            context,
            require_payment_link,
            metadata,
            customer,
            sign_in_link,
            sign_in_agent,
            sign_in_hint,
            country,
            end_user_ip,
            webhook_url,
            input_ttl_seconds,
            secrets,
            shops,
        )
        return await self._c._request(
            "POST", "/intents", json=body, idempotency_key=idempotency_key or str(uuid.uuid4()), wait=wait
        )

    async def get(self, intent_id: str, *, wait: int | None = None) -> Intent:
        return await self._c._request("GET", f"/intents/{intent_id}", wait=wait)

    async def list(
        self,
        *,
        status: IntentStatus | None = None,
        customer: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> IntentList:
        return await self._c._request(
            "GET",
            "/intents",
            params={"status": status, "customer": customer, "limit": limit, "cursor": cursor},
        )

    async def list_all(
        self, *, status: IntentStatus | None = None, customer: str | None = None, limit: int | None = None
    ) -> AsyncIterator[Intent]:
        cursor: str | None = None
        while True:
            page = await self.list(status=status, customer=customer, limit=limit, cursor=cursor)
            for intent in page.get("data", []):
                yield intent
            cursor = page.get("next_cursor")
            if not cursor:
                return

    async def cancel(self, intent_id: str, *, idempotency_key: str | None = None) -> Intent:
        return await self._c._request(
            "POST", f"/intents/{intent_id}/cancel", idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    async def input(
        self,
        intent_id: str,
        *,
        input: str,
        value: str | None = None,
        values: Mapping[str, str] | None = None,
        decline: bool = False,
        done: bool = False,
        idempotency_key: str | None = None,
    ) -> Intent:
        return await self._c._request(
            "POST",
            f"/intents/{intent_id}/input",
            json=_answer_body(input, value, decline, done, values),
            idempotency_key=idempotency_key or str(uuid.uuid4()),
        )

    async def list_events(
        self, intent_id: str, *, limit: int | None = None, cursor: str | None = None
    ) -> EventList:
        return await self._c._request(
            "GET", f"/intents/{intent_id}/events", params={"limit": limit, "cursor": cursor}
        )

    def stream_events(self, intent_id: str, *, last_event_id: str | None = None) -> AsyncIterator[Event]:
        """Follow the intent's timeline live: `async for event in oi.intents.stream_events(id)`."""
        return self._c._stream(f"/intents/{intent_id}/events", last_event_id)

    async def wait(
        self,
        intent_id: str,
        *,
        timeout: float = 300.0,
        interval: float = 2.0,
        return_on_payment: bool = True,
        return_on_sign_in: bool = True,
        return_on_input: bool = True,
    ) -> Intent:
        deadline = time.monotonic() + timeout
        while True:
            intent = await self.get(intent_id)
            if _settled(intent, return_on_payment, return_on_sign_in, return_on_input):
                return intent
            if time.monotonic() + interval > deadline:
                raise WaitTimeoutError(intent_id, timeout)
            await asyncio.sleep(interval)


class AsyncCustomers:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def create(
        self,
        *,
        external_id: str | None = None,
        name: str | None = None,
        email: str | None = None,
        metadata: Mapping[str, str] | None = None,
        limits: LimitsParams | None = None,
        idempotency_key: str | None = None,
    ) -> Customer:
        body = _fields(external_id=external_id, name=name, email=email, metadata=metadata, limits=limits)
        return await self._c._request(
            "POST", "/customers", json=body, idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    async def get(self, customer_id: str) -> Customer:
        return await self._c._request("GET", f"/customers/{customer_id}")

    async def update(
        self,
        customer_id: str,
        *,
        external_id: str | None = None,
        name: str | None = None,
        email: str | None = None,
        metadata: Mapping[str, str] | None = None,
        limits: LimitsParams | None = None,
    ) -> Customer:
        body = _fields(external_id=external_id, name=name, email=email, metadata=metadata, limits=limits)
        return await self._c._request("PATCH", f"/customers/{customer_id}", json=body)

    async def create_portal_session(
        self,
        customer_id: str,
        *,
        return_url: str | None = None,
        country: str | None = None,
        end_user_ip: str | None = None,
        ttl_seconds: int | None = None,
    ) -> PortalSession:
        """A link to a page where the customer sees and manages their own browser.

        They see the shops they are signed in to, can sign in to another, look
        around, and have their profile forgotten. Send url to them only; it works
        once. Until portal sessions are open to your key this raises a 503
        "portal_unavailable".
        """
        body = _fields(
            return_url=return_url, country=country, end_user_ip=end_user_ip, ttl_seconds=ttl_seconds
        )
        return await self._c._request("POST", f"/customers/{customer_id}/portal_sessions", json=body)

    async def create_secret_request(
        self,
        customer_id: str,
        *,
        kinds: Sequence[SecretKind],
        return_url: str | None = None,
        ttl_seconds: int | None = None,
    ) -> SecretRequestCreated:
        """Ask a customer to enter their own secrets (an address, a contact, an identity
        document) on a page of their own, so they never pass through you.

        Send them url; read the result with secret_requests.get or the
        "secret_request.completed" webhook.
        """
        body = _fields(kinds=list(kinds), return_url=return_url, ttl_seconds=ttl_seconds)
        return await self._c._request("POST", f"/customers/{customer_id}/secret_requests", json=body)

    async def list(
        self, *, external_id: str | None = None, limit: int | None = None, cursor: str | None = None
    ) -> CustomerList:
        return await self._c._request(
            "GET", "/customers", params={"external_id": external_id, "limit": limit, "cursor": cursor}
        )

    async def list_all(
        self, *, external_id: str | None = None, limit: int | None = None
    ) -> AsyncIterator[Customer]:
        cursor: str | None = None
        while True:
            page = await self.list(external_id=external_id, limit=limit, cursor=cursor)
            for customer in page.get("data", []):
                yield customer
            cursor = page.get("next_cursor")
            if not cursor:
                return


class AsyncProfiles:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def list(
        self, *, customer: str | None = None, limit: int | None = None, cursor: str | None = None
    ) -> ProfileList:
        return await self._c._request(
            "GET", "/profiles", params={"customer": customer, "limit": limit, "cursor": cursor}
        )

    async def list_all(
        self, *, customer: str | None = None, limit: int | None = None
    ) -> AsyncIterator[Profile]:
        cursor: str | None = None
        while True:
            page = await self.list(customer=customer, limit=limit, cursor=cursor)
            for profile in page.get("data", []):
                yield profile
            cursor = page.get("next_cursor")
            if not cursor:
                return

    async def create(self, *, customer: str | None = None) -> Profile:
        return await self._c._request("POST", "/profiles", json=_fields(customer=customer))

    async def get(self, profile_id: str) -> Profile:
        return await self._c._request("GET", f"/profiles/{profile_id}")

    async def forget(self, profile_id: str) -> Profile:
        return await self._c._request("DELETE", f"/profiles/{profile_id}")


class AsyncSessions:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def create(
        self,
        *,
        profile: str | None = None,
        customer: str | None = None,
        start_url: str | None = None,
        max_minutes: int | None = None,
        country: str | None = None,
        end_user_ip: str | None = None,
        webhook_url: str | None = None,
        idempotency_key: str | None = None,
    ) -> Session:
        body = _fields(
            profile=profile,
            customer=customer,
            start_url=start_url,
            max_minutes=max_minutes,
            country=country,
            end_user_ip=end_user_ip,
            webhook_url=webhook_url,
        )
        return await self._c._request(
            "POST", "/sessions", json=body, idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    async def get(self, session_id: str) -> Session:
        return await self._c._request("GET", f"/sessions/{session_id}")

    async def list(
        self,
        *,
        status: SessionStatus | None = None,
        profile: str | None = None,
        customer: str | None = None,
        limit: int | None = None,
        cursor: str | None = None,
    ) -> SessionList:
        return await self._c._request(
            "GET",
            "/sessions",
            params={
                "status": status,
                "profile": profile,
                "customer": customer,
                "limit": limit,
                "cursor": cursor,
            },
        )

    async def list_all(
        self,
        *,
        status: SessionStatus | None = None,
        profile: str | None = None,
        customer: str | None = None,
        limit: int | None = None,
    ) -> AsyncIterator[Session]:
        cursor: str | None = None
        while True:
            page = await self.list(
                status=status, profile=profile, customer=customer, limit=limit, cursor=cursor
            )
            for session in page.get("data", []):
                yield session
            cursor = page.get("next_cursor")
            if not cursor:
                return

    async def release(self, session_id: str, *, idempotency_key: str | None = None) -> Session:
        return await self._c._request(
            "POST", f"/sessions/{session_id}/release", idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    async def run_command(
        self, session_id: str, command: SessionCommand, *, idempotency_key: str | None = None
    ) -> SessionCommandResult:
        return await self._c._request(
            "POST",
            f"/sessions/{session_id}/commands",
            json=dict(command),
            idempotency_key=idempotency_key or str(uuid.uuid4()),
        )

    async def get_command(self, session_id: str, command_id: str) -> SessionCommandResult:
        return await self._c._request("GET", f"/sessions/{session_id}/commands/{command_id}")

    async def create_live_link(self, session_id: str) -> LiveLink:
        """A link to the session's live view. Whoever holds url sees the browser and drives it,
        though never to pay or place an order. Each call makes a new link, valid for 10 minutes.
        The session must be open."""
        return await self._c._request("POST", f"/sessions/{session_id}/live_links")

    async def screenshot(self, session_id: str) -> bytes:
        res = await self._c._send("GET", f"/sessions/{session_id}/screenshot", accept="image/jpeg")
        return res.content

    async def list_events(
        self, session_id: str, *, limit: int | None = None, cursor: str | None = None
    ) -> EventList:
        return await self._c._request(
            "GET", f"/sessions/{session_id}/events", params={"limit": limit, "cursor": cursor}
        )

    def stream_events(self, session_id: str, *, last_event_id: str | None = None) -> AsyncIterator[Event]:
        """Follow the session's timeline live: `async for event in oi.sessions.stream_events(id)`."""
        return self._c._stream(f"/sessions/{session_id}/events", last_event_id)
