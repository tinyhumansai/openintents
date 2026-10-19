"""Resources added with the v1 surface: secrets, sign-in links and webhook endpoints.

Each has a sync and an async twin. They only call the client's `_request`
and `_send`, so they work with either.
"""

from __future__ import annotations

import uuid
from collections.abc import AsyncIterator, Iterator, Mapping, Sequence
from typing import TYPE_CHECKING, Any

from .types import (
    Secret,
    SecretKind,
    SecretList,
    SecretRequest,
    SecretUseList,
    SignIn,
    WebhookDelivery,
    WebhookDeliveryList,
    WebhookEndpoint,
    WebhookEndpointList,
)

if TYPE_CHECKING:
    from ._client import AsyncOpenIntents, OpenIntents


def _fields(**fields: Any) -> dict[str, Any]:
    """The fields given: None means left out."""
    return {k: v for k, v in fields.items() if v is not None}


def _secret_body(
    kind: SecretKind,
    label: str,
    fields: Mapping[str, str],
    customer: str | None,
    shops: Sequence[str] | None,
    default: bool | None,
    expires_at: str | None,
) -> dict[str, Any]:
    return _fields(
        customer=customer,
        kind=kind,
        label=label,
        fields=dict(fields),
        shops=list(shops) if shops is not None else None,
        default=default,
        expires_at=expires_at,
    )


def _idempotent(key: str | None) -> str:
    return key or str(uuid.uuid4())


class Secrets:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def create(
        self,
        *,
        kind: SecretKind,
        label: str,
        fields: Mapping[str, str],
        customer: str | None = None,
        shops: Sequence[str] | None = None,
        default: bool | None = None,
        expires_at: str | None = None,
        idempotency_key: str | None = None,
    ) -> Secret:
        """Store details a purchase may need and that you would not put in a prompt.

        Secrets are write-only: one comes back as its label, a mask and a few
        public fields, never its values.
        """
        body = _secret_body(kind, label, fields, customer, shops, default, expires_at)
        return self._c._request("POST", "/secrets", json=body, idempotency_key=_idempotent(idempotency_key))

    def list(self, *, customer: str | None = None, kind: SecretKind | None = None) -> SecretList:
        """A payer's secrets in the key's mode, without their values: your account's, or a customer's."""
        return self._c._request("GET", "/secrets", params={"customer": customer, "kind": kind})

    def get(self, secret_id: str) -> Secret:
        return self._c._request("GET", f"/secrets/{secret_id}")

    def replace(
        self,
        secret_id: str,
        *,
        kind: SecretKind,
        label: str,
        fields: Mapping[str, str],
        customer: str | None = None,
        shops: Sequence[str] | None = None,
        default: bool | None = None,
        expires_at: str | None = None,
        idempotency_key: str | None = None,
    ) -> Secret:
        """Replace a secret's whole content; its version goes up."""
        body = _secret_body(kind, label, fields, customer, shops, default, expires_at)
        return self._c._request(
            "PUT", f"/secrets/{secret_id}", json=body, idempotency_key=_idempotent(idempotency_key)
        )

    def delete(self, secret_id: str) -> None:
        """Delete a secret and its values."""
        self._c._send("DELETE", f"/secrets/{secret_id}")

    def uses(self, secret_id: str, *, limit: int | None = None, cursor: str | None = None) -> SecretUseList:
        """A secret's history, newest first: made, replaced, filled into a shop or refused. Never a value."""
        return self._c._request(
            "GET", f"/secrets/{secret_id}/uses", params={"limit": limit, "cursor": cursor}
        )


class SecretRequests:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def get(self, request_id: str) -> SecretRequest:
        """Where a request for a customer's secrets stands. Make one with customers.create_secret_request."""
        return self._c._request("GET", f"/secret_requests/{request_id}")


class SignIns:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def create(
        self,
        *,
        shop_url: str,
        customer: str | None = None,
        country: str | None = None,
        end_user_ip: str | None = None,
        return_url: str | None = None,
        idempotency_key: str | None = None,
    ) -> SignIn:
        """A link where the payer signs in to a shop with no intent attached; the profile keeps the login.

        Send them url. Its end is a "sign_in.completed" or "sign_in.expired" webhook.
        """
        body = _fields(
            shop_url=shop_url,
            customer=customer,
            country=country,
            end_user_ip=end_user_ip,
            return_url=return_url,
        )
        return self._c._request("POST", "/sign_ins", json=body, idempotency_key=_idempotent(idempotency_key))

    def get(self, sign_in_id: str) -> SignIn:
        return self._c._request("GET", f"/sign_ins/{sign_in_id}")


class WebhookEndpoints:
    def __init__(self, client: OpenIntents) -> None:
        self._c = client

    def create(
        self,
        *,
        url: str,
        events: Sequence[str] | None = None,
        idempotency_key: str | None = None,
    ) -> WebhookEndpoint:
        """Register a URL for your account's events. The response alone carries its secret: keep it."""
        body = _fields(url=url, events=list(events) if events is not None else None)
        return self._c._request(
            "POST", "/webhook_endpoints", json=body, idempotency_key=_idempotent(idempotency_key)
        )

    def list(self, *, limit: int | None = None, cursor: str | None = None) -> WebhookEndpointList:
        """Newest first, without their secrets."""
        return self._c._request("GET", "/webhook_endpoints", params={"limit": limit, "cursor": cursor})

    def list_all(self, *, limit: int | None = None) -> Iterator[WebhookEndpoint]:
        """Iterate every endpoint, following next_cursor."""
        cursor: str | None = None
        while True:
            page = self.list(limit=limit, cursor=cursor)
            yield from page.get("data", [])
            cursor = page.get("next_cursor")
            if not cursor:
                return

    def get(self, endpoint_id: str) -> WebhookEndpoint:
        return self._c._request("GET", f"/webhook_endpoints/{endpoint_id}")

    def update(
        self,
        endpoint_id: str,
        *,
        url: str | None = None,
        events: Sequence[str] | None = None,
        disabled: bool | None = None,
    ) -> WebhookEndpoint:
        """Change its URL, its event types (events=[] is every intent event) or whether it is disabled."""
        body = _fields(url=url, events=list(events) if events is not None else None, disabled=disabled)
        return self._c._request("PATCH", f"/webhook_endpoints/{endpoint_id}", json=body)

    def delete(self, endpoint_id: str) -> None:
        self._c._send("DELETE", f"/webhook_endpoints/{endpoint_id}")

    def rotate_secret(self, endpoint_id: str) -> WebhookEndpoint:
        """Make a new secret, returned here only. The one before keeps signing for 24 hours;
        verify_webhook accepts a header that carries both."""
        return self._c._request("POST", f"/webhook_endpoints/{endpoint_id}/rotate_secret")

    def test(self, endpoint_id: str) -> WebhookDelivery:
        """Send a "webhook_endpoint.test" event to this endpoint alone; the delivery shows how it goes."""
        return self._c._request("POST", f"/webhook_endpoints/{endpoint_id}/test")

    def list_deliveries(
        self, endpoint_id: str, *, limit: int | None = None, cursor: str | None = None
    ) -> WebhookDeliveryList:
        """What was sent to the endpoint, newest first."""
        return self._c._request(
            "GET", f"/webhook_endpoints/{endpoint_id}/deliveries", params={"limit": limit, "cursor": cursor}
        )

    def redeliver(self, endpoint_id: str, delivery_id: str) -> WebhookDelivery:
        """Send a delivery again, with the same event id and body."""
        return self._c._request(
            "POST", f"/webhook_endpoints/{endpoint_id}/deliveries/{delivery_id}/redeliver"
        )


class AsyncSecrets:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def create(
        self,
        *,
        kind: SecretKind,
        label: str,
        fields: Mapping[str, str],
        customer: str | None = None,
        shops: Sequence[str] | None = None,
        default: bool | None = None,
        expires_at: str | None = None,
        idempotency_key: str | None = None,
    ) -> Secret:
        body = _secret_body(kind, label, fields, customer, shops, default, expires_at)
        return await self._c._request(
            "POST", "/secrets", json=body, idempotency_key=_idempotent(idempotency_key)
        )

    async def list(self, *, customer: str | None = None, kind: SecretKind | None = None) -> SecretList:
        return await self._c._request("GET", "/secrets", params={"customer": customer, "kind": kind})

    async def get(self, secret_id: str) -> Secret:
        return await self._c._request("GET", f"/secrets/{secret_id}")

    async def replace(
        self,
        secret_id: str,
        *,
        kind: SecretKind,
        label: str,
        fields: Mapping[str, str],
        customer: str | None = None,
        shops: Sequence[str] | None = None,
        default: bool | None = None,
        expires_at: str | None = None,
        idempotency_key: str | None = None,
    ) -> Secret:
        body = _secret_body(kind, label, fields, customer, shops, default, expires_at)
        return await self._c._request(
            "PUT", f"/secrets/{secret_id}", json=body, idempotency_key=_idempotent(idempotency_key)
        )

    async def delete(self, secret_id: str) -> None:
        await self._c._send("DELETE", f"/secrets/{secret_id}")

    async def uses(
        self, secret_id: str, *, limit: int | None = None, cursor: str | None = None
    ) -> SecretUseList:
        return await self._c._request(
            "GET", f"/secrets/{secret_id}/uses", params={"limit": limit, "cursor": cursor}
        )


class AsyncSecretRequests:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def get(self, request_id: str) -> SecretRequest:
        return await self._c._request("GET", f"/secret_requests/{request_id}")


class AsyncSignIns:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def create(
        self,
        *,
        shop_url: str,
        customer: str | None = None,
        country: str | None = None,
        end_user_ip: str | None = None,
        return_url: str | None = None,
        idempotency_key: str | None = None,
    ) -> SignIn:
        body = _fields(
            shop_url=shop_url,
            customer=customer,
            country=country,
            end_user_ip=end_user_ip,
            return_url=return_url,
        )
        return await self._c._request(
            "POST", "/sign_ins", json=body, idempotency_key=_idempotent(idempotency_key)
        )

    async def get(self, sign_in_id: str) -> SignIn:
        return await self._c._request("GET", f"/sign_ins/{sign_in_id}")


class AsyncWebhookEndpoints:
    def __init__(self, client: AsyncOpenIntents) -> None:
        self._c = client

    async def create(
        self,
        *,
        url: str,
        events: Sequence[str] | None = None,
        idempotency_key: str | None = None,
    ) -> WebhookEndpoint:
        body = _fields(url=url, events=list(events) if events is not None else None)
        return await self._c._request(
            "POST", "/webhook_endpoints", json=body, idempotency_key=_idempotent(idempotency_key)
        )

    async def list(self, *, limit: int | None = None, cursor: str | None = None) -> WebhookEndpointList:
        return await self._c._request("GET", "/webhook_endpoints", params={"limit": limit, "cursor": cursor})

    async def list_all(self, *, limit: int | None = None) -> AsyncIterator[WebhookEndpoint]:
        cursor: str | None = None
        while True:
            page = await self.list(limit=limit, cursor=cursor)
            for endpoint in page.get("data", []):
                yield endpoint
            cursor = page.get("next_cursor")
            if not cursor:
                return

    async def get(self, endpoint_id: str) -> WebhookEndpoint:
        return await self._c._request("GET", f"/webhook_endpoints/{endpoint_id}")

    async def update(
        self,
        endpoint_id: str,
        *,
        url: str | None = None,
        events: Sequence[str] | None = None,
        disabled: bool | None = None,
    ) -> WebhookEndpoint:
        body = _fields(url=url, events=list(events) if events is not None else None, disabled=disabled)
        return await self._c._request("PATCH", f"/webhook_endpoints/{endpoint_id}", json=body)

    async def delete(self, endpoint_id: str) -> None:
        await self._c._send("DELETE", f"/webhook_endpoints/{endpoint_id}")

    async def rotate_secret(self, endpoint_id: str) -> WebhookEndpoint:
        return await self._c._request("POST", f"/webhook_endpoints/{endpoint_id}/rotate_secret")

    async def test(self, endpoint_id: str) -> WebhookDelivery:
        return await self._c._request("POST", f"/webhook_endpoints/{endpoint_id}/test")

    async def list_deliveries(
        self, endpoint_id: str, *, limit: int | None = None, cursor: str | None = None
    ) -> WebhookDeliveryList:
        return await self._c._request(
            "GET", f"/webhook_endpoints/{endpoint_id}/deliveries", params={"limit": limit, "cursor": cursor}
        )

    async def redeliver(self, endpoint_id: str, delivery_id: str) -> WebhookDelivery:
        return await self._c._request(
            "POST", f"/webhook_endpoints/{endpoint_id}/deliveries/{delivery_id}/redeliver"
        )
