"""Sync and async clients for the OpenIntents agentic API."""

from __future__ import annotations

import asyncio
import os
import random
import time
import uuid
from collections.abc import AsyncIterator, Iterator, Mapping
from typing import Any

import httpx

from ._version import __version__
from .errors import OpenIntentsError, WaitTimeoutError
from .types import FINAL_STATUSES, Balance, Intent, IntentList, IntentStatus

DEFAULT_BASE_URL = "https://api.openintents.io/v1"


def _resolve(api_key: str | None, base_url: str | None) -> tuple[str, str]:
    key = api_key or os.environ.get("OPENINTENTS_API_KEY")
    if not key:
        raise ValueError("Missing API key: pass api_key= or set OPENINTENTS_API_KEY.")
    url = (base_url or os.environ.get("OPENINTENTS_BASE_URL") or DEFAULT_BASE_URL).rstrip("/")
    return key, url


def _headers(api_key: str, idempotency_key: str | None) -> dict[str, str]:
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Accept": "application/json",
        "User-Agent": f"openintents-python/{__version__}",
    }
    if idempotency_key:
        headers["Idempotency-Key"] = idempotency_key
    return headers


def _backoff(attempt: int, retry_after: str | None) -> float:
    try:
        if retry_after is not None and float(retry_after) > 0:
            return float(retry_after)
    except ValueError:
        pass
    return min(8.0, 0.5 * 2**attempt) * random.uniform(0.75, 1.25)


def _retryable(status: int) -> bool:
    return status == 429 or status >= 500


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
        request_id=res.headers.get("request-id"),
    )


def _create_body(
    intent: str,
    max_amount: int | None,
    currency: str | None,
    context: Mapping[str, Any] | None,
    require_payment_link: bool | None,
    metadata: Mapping[str, str] | None,
) -> dict[str, Any]:
    body: dict[str, Any] = {"intent": intent}
    for k, v in (
        ("max_amount", max_amount),
        ("currency", currency),
        ("context", dict(context) if context is not None else None),
        ("require_payment_link", require_payment_link),
        ("metadata", dict(metadata) if metadata is not None else None),
    ):
        if v is not None:
            body[k] = v
    return body


def _settled(intent: Intent, return_on_payment: bool) -> bool:
    status = intent.get("status")
    return status in FINAL_STATUSES or (return_on_payment and status == "requires_payment")


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
        self._http = http_client or httpx.Client(timeout=timeout)
        self.intents = Intents(self)
        self.balance = BalanceResource(self)

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
    ) -> Any:
        query = {k: v for k, v in (params or {}).items() if v is not None}
        headers = _headers(self._api_key, idempotency_key)
        attempt = 0
        while True:
            try:
                res = self._http.request(
                    method, self._base_url + path, json=json, params=query, headers=headers
                )
            except httpx.TransportError as exc:
                if attempt < self._max_retries:
                    time.sleep(_backoff(attempt, None))
                    attempt += 1
                    continue
                raise OpenIntentsError(f"Network error: {exc}", status=0, type="network") from exc
            if res.is_success:
                return res.json()
            if _retryable(res.status_code) and attempt < self._max_retries:
                time.sleep(_backoff(attempt, res.headers.get("retry-after")))
                attempt += 1
                continue
            raise _error(res)


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
        idempotency_key: str | None = None,
    ) -> Intent:
        """Start an intent. Returns immediately, usually with status "queued"."""
        body = _create_body(intent, max_amount, currency, context, require_payment_link, metadata)
        return self._c._request(
            "POST", "/intents", json=body, idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    def get(self, intent_id: str) -> Intent:
        return self._c._request("GET", f"/intents/{intent_id}")

    def list(
        self, *, status: IntentStatus | None = None, limit: int | None = None, cursor: str | None = None
    ) -> IntentList:
        return self._c._request(
            "GET", "/intents", params={"status": status, "limit": limit, "cursor": cursor}
        )

    def list_all(self, *, status: IntentStatus | None = None, limit: int | None = None) -> Iterator[Intent]:
        """Iterate every intent matching the filters, following next_cursor."""
        cursor: str | None = None
        while True:
            page = self.list(status=status, limit=limit, cursor=cursor)
            yield from page.get("data", [])
            cursor = page.get("next_cursor")
            if not cursor:
                return

    def cancel(self, intent_id: str, *, idempotency_key: str | None = None) -> Intent:
        """Cancel an intent that hasn't paid yet. Cancelled intents are free."""
        return self._c._request(
            "POST", f"/intents/{intent_id}/cancel", idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    def wait(
        self, intent_id: str, *, timeout: float = 300.0, interval: float = 2.0, return_on_payment: bool = True
    ) -> Intent:
        """Poll until the intent is final, or (by default) needs payment.

        On "requires_payment", hand intent["payment"]["payment_url"] to a person.
        """
        deadline = time.monotonic() + timeout
        while True:
            intent = self.get(intent_id)
            if _settled(intent, return_on_payment):
                return intent
            if time.monotonic() + interval > deadline:
                raise WaitTimeoutError(intent_id, timeout)
            time.sleep(interval)


class BalanceResource:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def get(self) -> Balance:
        """The agent balance intents pay from, and this month's spend."""
        return self._c._request("GET", "/balance")


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
        self._http = http_client or httpx.AsyncClient(timeout=timeout)
        self.intents = AsyncIntents(self)
        self.balance = AsyncBalanceResource(self)

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
    ) -> Any:
        query = {k: v for k, v in (params or {}).items() if v is not None}
        headers = _headers(self._api_key, idempotency_key)
        attempt = 0
        while True:
            try:
                res = await self._http.request(
                    method, self._base_url + path, json=json, params=query, headers=headers
                )
            except httpx.TransportError as exc:
                if attempt < self._max_retries:
                    await asyncio.sleep(_backoff(attempt, None))
                    attempt += 1
                    continue
                raise OpenIntentsError(f"Network error: {exc}", status=0, type="network") from exc
            if res.is_success:
                return res.json()
            if _retryable(res.status_code) and attempt < self._max_retries:
                await asyncio.sleep(_backoff(attempt, res.headers.get("retry-after")))
                attempt += 1
                continue
            raise _error(res)


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
        idempotency_key: str | None = None,
    ) -> Intent:
        body = _create_body(intent, max_amount, currency, context, require_payment_link, metadata)
        return await self._c._request(
            "POST", "/intents", json=body, idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    async def get(self, intent_id: str) -> Intent:
        return await self._c._request("GET", f"/intents/{intent_id}")

    async def list(
        self, *, status: IntentStatus | None = None, limit: int | None = None, cursor: str | None = None
    ) -> IntentList:
        return await self._c._request(
            "GET", "/intents", params={"status": status, "limit": limit, "cursor": cursor}
        )

    async def list_all(
        self, *, status: IntentStatus | None = None, limit: int | None = None
    ) -> AsyncIterator[Intent]:
        cursor: str | None = None
        while True:
            page = await self.list(status=status, limit=limit, cursor=cursor)
            for intent in page.get("data", []):
                yield intent
            cursor = page.get("next_cursor")
            if not cursor:
                return

    async def cancel(self, intent_id: str, *, idempotency_key: str | None = None) -> Intent:
        return await self._c._request(
            "POST", f"/intents/{intent_id}/cancel", idempotency_key=idempotency_key or str(uuid.uuid4())
        )

    async def wait(
        self, intent_id: str, *, timeout: float = 300.0, interval: float = 2.0, return_on_payment: bool = True
    ) -> Intent:
        deadline = time.monotonic() + timeout
        while True:
            intent = await self.get(intent_id)
            if _settled(intent, return_on_payment):
                return intent
            if time.monotonic() + interval > deadline:
                raise WaitTimeoutError(intent_id, timeout)
            await asyncio.sleep(interval)


class AsyncBalanceResource:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def get(self) -> Balance:
        return await self._c._request("GET", "/balance")
