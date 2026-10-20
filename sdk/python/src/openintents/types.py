"""Types for the OpenIntents v1 API. Source of truth: spec/openapi.yaml.

Responses are returned as plain dicts typed with TypedDicts, so new fields
from the API never break older SDK versions.
"""

from __future__ import annotations

from typing import Any, Literal, TypedDict

IntentStatus = Literal[
    "queued",
    "running",
    "requires_payment",
    "requires_input",
    "requires_login",
    "completed",
    "failed",
    "cancelled",
]

#: Statuses an intent never leaves.
FINAL_STATUSES: frozenset[str] = frozenset({"completed", "failed", "cancelled"})

#: Why a browser goes out from its country: you named it ("explicit"), it is
#: the country the payer last browsed from ("payer"), it was inferred from the
#: end_user_ip you gave ("end_user_ip") or from the address the payer's own page
#: was opened from ("request_ip"), or it is the default ("default"). Others may
#: appear.
CountrySource = Literal["explicit", "payer", "end_user_ip", "request_ip", "default"]


class RateLimit(TypedDict):
    """Your key's limit, from the RateLimit-* headers of a response."""

    #: Requests a minute your key may make; also the burst.
    limit: int
    #: Requests left in your bucket.
    remaining: int
    #: Seconds until your bucket is full again.
    reset: int


class PaymentRequest(TypedDict, total=False):
    reason: Literal[
        "over_limit",
        "requested",
        "connect_wallet",
        "add_money",
        "identity_check",
        "wallet_processing",
    ]
    payment_url: str
    merchant: str | None
    total: int
    fee: int
    currency: str
    expires_at: str


class SignInRequest(TypedDict, total=False):
    """Where the payer signs in to a shop, so a waiting intent carries on.

    Send url to the payer only: it works for 30 minutes, on the first device
    that opens it.
    """

    url: str
    #: The shop's domain, such as "amazon.com".
    shop: str
    expires_at: str


class SignInHint(TypedDict, total=False):
    """The payer's phone number or email for the agent to sign in with.

    Kept sealed, used once, deleted when the intent ends, and never returned.
    """

    #: Digits, with an optional leading "+"; spaces, dashes, dots and parentheses are fine.
    phone: str
    email: str


#: "phone" or "email" to sign in with, a one-time "code" the shop sent, the
#: payer's "approval" of the sign-in in the shop's app or on another device
#: (it has no value: answer done=True), or a "detail": anything else the
#: purchase needs that the request did not give, such as the email the shop
#: wants for the order, a size or a colour. A detail's prompt is the agent's
#: own question (never asking for passwords, codes, PINs, card or bank details
#: or ID numbers), and the answer goes back to the agent.
#: A "sensitive" question wants an identity document: the payer answers it on
#: its url only (anything else but a decline is a 422 answer_on_page).
#: A "payment" asks the payer to pay the shop themselves (pay_at_shop): show
#: them image_url, a picture of the shop's payment code to scan with a UPI
#: app, and answer done=True once they paid (it has no value), or decline.
InputKind = Literal["phone", "email", "code", "approval", "detail", "sensitive", "payment"]


class _InputChoiceRequired(TypedDict):
    #: What an answer names; unique in the question.
    id: str
    label: str


class InputChoice(_InputChoiceRequired, total=False):
    """One of the choices a question offers, such as a seat, a fare or a delivery slot."""

    description: str
    #: What choosing it adds to the price (negative, takes off), in minor units of currency.
    price_delta: int
    currency: str


#: What a field of a form takes.
InputFieldType = Literal["text", "email", "phone", "date", "number", "choice"]


class InputField(TypedDict, total=False):
    """One field of a form a question asks the payer to fill in."""

    #: The key of this field's answer in values.
    name: str
    label: str
    type: InputFieldType
    required: bool
    #: For a "choice" field, its options; answer with an option's id.
    options: list[InputChoice]


class InputRequest(TypedDict, total=False):
    """What the agent needs from the payer while it signs them in, or a detail.

    Asked while it signs them in to a shop itself, or for a "detail" the
    purchase needs (at most 3 per intent). Ask them prompt and answer with
    intents.input right away: it expires after 5 minutes. Or send them url to
    type the answer there.
    """

    #: Answer with this ID.
    id: str
    kind: InputKind
    #: The shop's registrable domain, such as "blinkit.com".
    shop: str
    #: What to ask the payer, written by OpenIntents (for a "detail", the
    #: agent's own question). Show it to them as it is.
    prompt: str
    #: A "detail"'s choices (at most 8), empty otherwise. When not empty, answer
    #: with one of them (case-insensitive): anything else is a 400.
    options: list[str]
    #: A "detail" that offers choices with ids (2 to 50), empty otherwise. Answer
    #: with the id of one as value=; any other is a 422 unknown_option.
    choices: list[InputChoice]
    #: A "detail" that is a form (at most 10), empty otherwise. Answer every
    #: field at once with values=.
    fields: list[InputField]
    #: What a plain answer must look like.
    expects: Literal["text", "email", "phone", "choice"]
    #: Where the shop sent the code, when known.
    channel: Literal["sms", "email", "call", "app"] | None
    #: How many characters the code has, when the page says.
    length: int | None
    #: A page where the payer types the answer themselves. Theirs only: whoever has it can answer.
    url: str
    #: A "payment"'s picture (JPEG) of the shop's payment code, for the payer to
    #: scan; served only while the request waits, absent for other kinds. Like
    #: url, whoever has it can see it.
    image_url: str
    expires_at: str
    #: How many more times the agent may ask for this kind in this purchase, after this one.
    attempts_left: int


class ReceiptItem(TypedDict):
    name: str
    quantity: int
    amount: int


class Receipt(TypedDict, total=False):
    merchant: str
    items: list[ReceiptItem]
    total: int
    fee: int
    currency: str
    confirmation: str | None
    receipt_url: str | None


class IntentError(TypedDict):
    code: str
    message: str


class Intent(TypedDict, total=False):
    id: str
    object: Literal["intent"]
    status: IntentStatus
    intent: str
    customer: str | None
    max_amount: int | None
    currency: str | None
    context: dict[str, Any] | None
    metadata: dict[str, str] | None
    payment: PaymentRequest | None
    sign_in: SignInRequest | None
    input_request: InputRequest | None
    #: Where this intent's own events go, if it was created with a webhook_url.
    webhook_url: str | None
    #: Signs the webhook_url deliveries ("whsec_..."). Only in the create response.
    webhook_secret: str
    #: The secrets the purchase may use, as given (never a value); None when none were named.
    secrets: list[str] | None
    #: The shops you let the purchase fill secrets on.
    shops: list[str]
    #: The country its browser goes out from.
    country: str | None
    country_source: CountrySource | None
    result: Receipt | None
    error: IntentError | None
    test: bool
    created_at: str
    completed_at: str | None


class IntentList(TypedDict, total=False):
    object: Literal["list"]
    data: list[Intent]
    next_cursor: str | None


class Limits(TypedDict):
    """What a customer may spend, in cents, counting the cart's total of each purchase their wallet paid.

    None is no limit. Checked before anything is paid; an intent over one fails with limit_exceeded.
    """

    per_intent: int | None
    monthly: int | None


class LimitsParams(TypedDict, total=False):
    """Limits to set: one left out stays; a number sets it; None removes it (1 to 1000000000)."""

    per_intent: int | None
    monthly: int | None


class Customer(TypedDict, total=False):
    """Someone you buy for, with their own wallet and browser profile."""

    id: str
    object: Literal["customer"]
    external_id: str | None
    name: str | None
    email: str | None
    metadata: dict[str, str]
    limits: Limits
    created_at: str


class CustomerList(TypedDict, total=False):
    object: Literal["list"]
    data: list[Customer]
    next_cursor: str | None


class Event(TypedDict, total=False):
    """One step in an intent's or a session's timeline.

    `type` says what `data` holds: "status" ({status, reason?}), "step" and
    "checkout" for intents, "page" and "command" for sessions. New types may
    appear: skip the ones you don't know.
    """

    id: str
    object: Literal["event"]
    type: str
    created_at: str
    data: dict[str, Any]


class EventList(TypedDict, total=False):
    object: Literal["list"]
    data: list[Event]
    next_cursor: str | None


#: "ready": nothing uses it. "in_use": an intent or a session holds it (see
#: lease). "forgetting": being erased. "forgotten": erased, nothing saved since.
ProfileStatus = Literal["ready", "in_use", "forgetting", "forgotten"]


class LeaseHolder(TypedDict):
    type: str
    id: str


class ProfileLease(TypedDict):
    """What uses a profile now. A profile is used by one intent or session at a time."""

    holder: LeaseHolder
    expires_at: str


class Profile(TypedDict, total=False):
    """A payer's browser profile: the merchant logins, cookies and carts they build up."""

    id: str
    object: Literal["profile"]
    customer: str | None
    status: ProfileStatus
    lease: ProfileLease | None
    size_bytes: int | None
    created_at: str
    last_used_at: str | None


class ProfileList(TypedDict, total=False):
    object: Literal["list"]
    data: list[Profile]
    next_cursor: str | None


SessionStatus = Literal["queued", "opening", "open", "closing", "closed", "failed"]


class Session(TypedDict, total=False):
    """A browser opened on a profile without an intent."""

    id: str
    object: Literal["session"]
    profile: str
    customer: str | None
    status: SessionStatus
    test: bool
    start_url: str | None
    url: str | None
    title: str | None
    created_at: str
    opened_at: str | None
    expires_at: str
    closed_at: str | None
    #: The country its browser goes out from.
    country: str | None
    country_source: CountrySource | None
    #: Where this session's own events go, if it was opened with a webhook_url.
    webhook_url: str | None
    #: Signs the webhook_url deliveries ("whsec_..."). Only in the create response.
    webhook_secret: str
    #: "released", "idle", "timed_out", "data_limit", "browser_lost" or "error"; others may appear.
    close_reason: str | None


class SessionList(TypedDict, total=False):
    object: Literal["list"]
    data: list[Session]
    next_cursor: str | None


class NavigateCommand(TypedDict):
    """Go to a public https URL; other hosts and ports are refused (url_not_allowed)."""

    type: Literal["navigate"]
    url: str


class SnapshotCommand(TypedDict):
    """The page's accessibility tree, with a ref on each element you can act on."""

    type: Literal["snapshot"]


class _ActCommand(TypedDict):
    type: Literal["act"]
    ref: str
    action: Literal["click", "type", "select", "press"]


class ActCommand(_ActCommand, total=False):
    """Act on an element by its ref from the latest snapshot.

    "type" takes `text` (text fields only; line breaks but no other control
    characters), "select" takes `value`, "press" takes `key` (one key, alone
    or with Shift or Control: "Enter", "Shift+Tab", "Control+a"). After a
    navigate, and after any act but a "type" without line breaks, take a new
    snapshot before the next act.
    """

    text: str
    value: str
    key: str


class _ReadCommand(TypedDict):
    type: Literal["read"]


class ReadCommand(_ReadCommand, total=False):
    """The page's readable text, or one element's with a CSS selector."""

    selector: str


#: One thing to do on a session's page. A session never pays or places an order.
SessionCommand = NavigateCommand | SnapshotCommand | ActCommand | ReadCommand

SessionCommandStatus = Literal["pending", "done", "refused", "failed"]


class Failure(TypedDict):
    """Why a session command was refused or failed, with a code such as payment_not_allowed."""

    code: str
    message: str


class SessionCommandResult(TypedDict, total=False):
    id: str
    object: Literal["session_command"]
    type: str
    status: SessionCommandStatus
    url: str | None
    title: str | None
    #: For a snapshot, the tree. It never includes what text fields hold.
    snapshot: str | None
    #: For a read, the text.
    text: str | None
    error: Failure | None
    created_at: str
    completed_at: str | None


IntentWebhookEventType = Literal[
    "intent.running",
    "intent.requires_payment",
    "intent.requires_input",
    "intent.requires_login",
    "intent.completed",
    "intent.failed",
    "intent.cancelled",
]

#: Sent only to endpoints subscribed to session.* types.
SessionWebhookEventType = Literal["session.opened", "session.closed", "session.failed"]

#: Sent only to endpoints subscribed to sign_in.* types.
SignInWebhookEventType = Literal["sign_in.completed", "sign_in.expired"]

#: Sent only to endpoints subscribed to it.
SecretRequestWebhookEventType = Literal["secret_request.completed"]


class IntentWebhookEvent(TypedDict):
    id: str
    type: IntentWebhookEventType
    created_at: str
    data: Intent


class SessionWebhookEvent(TypedDict):
    id: str
    type: SessionWebhookEventType
    created_at: str
    data: Session


class SignInWebhookEvent(TypedDict):
    """A sign-in link ended. data is the sign-in without its url."""

    id: str
    type: SignInWebhookEventType
    created_at: str
    data: SignIn


class SecretRequestWebhookEvent(TypedDict):
    """A customer filled in a secret request. data holds the handles of the secrets made, never a value."""

    id: str
    type: SecretRequestWebhookEventType
    created_at: str
    data: SecretRequest


class WebhookTestEventData(TypedDict):
    endpoint: str
    message: str


class WebhookTestEvent(TypedDict):
    """What webhook_endpoints.test sends, to that endpoint alone. Not a type you can subscribe to."""

    id: str
    type: Literal["webhook_endpoint.test"]
    created_at: str
    data: WebhookTestEventData


#: Dispatch on "type" before reading "data".
WebhookEvent = (
    IntentWebhookEvent
    | SessionWebhookEvent
    | SignInWebhookEvent
    | SecretRequestWebhookEvent
    | WebhookTestEvent
)


class PortalSession(TypedDict):
    """A link to a customer's own page, shown once. Send url to the customer only."""

    id: str
    object: Literal["portal_session"]
    customer: str
    #: The country the customer's browser goes out from.
    country: str
    country_source: CountrySource
    #: Works once, until expires_at.
    url: str
    expires_at: str


#: What a secret holds, and so which fields it takes.
SecretKind = Literal["address", "contact", "identity_document", "shop_password", "loyalty"]


class Secret(TypedDict):
    """A stored secret as you may see it: never its values."""

    id: str
    object: Literal["secret"]
    #: The customer it belongs to; None for your account's own.
    customer: str | None
    kind: SecretKind
    label: str
    #: What a person may be shown to recognise it, such as "Paris, FR" for an address.
    mask: str
    #: The few fields its kind keeps in the clear.
    public: dict[str, str]
    shops: list[str]
    default: bool
    #: The mode of the key that wrote it. A key sees only secrets of its own mode.
    mode: Literal["test", "live"]
    #: "developer" through this API, "payer" on their own page.
    entered_by: Literal["developer", "payer"]
    #: Goes up each time it is replaced.
    version: int
    created_at: str
    updated_at: str
    expires_at: str | None
    last_used_at: str | None


class SecretList(TypedDict, total=False):
    object: Literal["list"]
    data: list[Secret]
    next_cursor: str | None


class SecretUse(TypedDict, total=False):
    """One row of a secret's history. Never a value or a mask."""

    id: str
    object: Literal["secret_use"]
    secret: str
    kind: SecretKind
    #: "create", "replace", "delete", "expire", "fill", "fund" or "refuse"; others may appear.
    action: str
    #: "worker", "api", "payer", "developer" or "system".
    actor: str
    outcome: Literal["ok", "refused"]
    #: Why a use was refused: "shop_not_allowed", "expired" or "wrong_use".
    reason: str | None
    field: str | None
    #: The names of the fields a create or replace set.
    field_names: list[str]
    shop: str | None
    intent: str | None
    at: str


class SecretUseList(TypedDict, total=False):
    object: Literal["list"]
    data: list[SecretUse]
    next_cursor: str | None


class SecretRequestCreated(TypedDict):
    """A request for a customer's secrets, with the link to send them."""

    id: str
    object: Literal["secret_request"]
    customer: str
    kinds: list[SecretKind]
    #: For the customer only. Works once, until expires_at.
    url: str
    expires_at: str


class SecretRequest(TypedDict):
    """Where a request for a customer's secrets stands; also the data of secret_request.completed."""

    id: str
    object: Literal["secret_request"]
    customer: str
    mode: Literal["test", "live"]
    kinds: list[SecretKind]
    #: "pending" until the customer fills it in, then "done"; "expired" if they did not in time.
    state: Literal["pending", "done", "expired"]
    expires_at: str
    completed_at: str | None
    #: The handles ("sec_...") of the secrets the customer made.
    secrets: list[str]


class LiveLink(TypedDict):
    """A link to a session's live view, shown once. Whoever holds url can drive the session."""

    object: Literal["live_link"]
    session: str
    url: str
    #: 10 minutes after it was made.
    expires_at: str


#: "open" until the payer opens it, "active" while their browser is open, then
#: "done" (they signed in), "declined", "expired" or "cancelled".
SignInStatus = Literal["open", "active", "done", "declined", "expired", "cancelled"]


class SignIn(TypedDict, total=False):
    """A link where a payer signs in to a shop with no intent attached."""

    id: str
    object: Literal["sign_in"]
    status: SignInStatus
    customer: str | None
    #: The shop's domain, such as "amazon.com".
    shop: str
    country: str | None
    country_source: CountrySource | None
    return_url: str | None
    #: The page for the payer only. Not in webhooks.
    url: str
    test: bool
    created_at: str
    expires_at: str
    ended_at: str | None


class WebhookEndpoint(TypedDict, total=False):
    id: str
    object: Literal["webhook_endpoint"]
    url: str
    #: The event types it receives. Empty: every intent event.
    events: list[str]
    disabled: bool
    created_at: str
    #: Present while the secret before the last rotation still signs as well.
    previous_secret_expires_at: str
    #: The signing secret ("whsec_..."). Only in the create and rotate responses.
    secret: str


class WebhookEndpointList(TypedDict, total=False):
    object: Literal["list"]
    data: list[WebhookEndpoint]
    next_cursor: str | None


class WebhookDelivery(TypedDict, total=False):
    """One send of an event to an endpoint."""

    id: str
    object: Literal["webhook_delivery"]
    endpoint: str | None
    event_id: str
    event_type: str
    #: "pending" while it is being tried (and retried), then "delivered" or "failed".
    status: Literal["pending", "delivered", "failed"]
    attempts: int
    #: What your receiver answered on the last attempt, if it answered.
    response_code: int | None
    #: Why the last attempt failed, if it got no answer.
    error: str | None
    created_at: str
    #: The next attempt. Only while pending.
    next_attempt_at: str
    delivered_at: str | None
    #: The delivery this one sends again.
    redelivery_of: str | None


class WebhookDeliveryList(TypedDict, total=False):
    object: Literal["list"]
    data: list[WebhookDelivery]
    next_cursor: str | None
