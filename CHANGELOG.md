# Changelog

All three SDKs share one version. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- API: with `pay_at_shop`, the agent takes cash on delivery where the shop
  offers it, before the shop's UPI QR code; a `payment` question carries
  `upi_url`, the same payment as a `upi://pay` link to open in a UPI app on
  the phone, when the shop's page gave one; and the receipt's `paid_with`
  says how the shop was paid (`wallet`, `cash_on_delivery`, `upi_qr`).
- SDKs: `upi_url` on `InputRequest` and `paid_with` on `Receipt` (TypeScript,
  Python, Rust); the Rust CLI's `show` prints the link.
- API: a ride (Uber, Ola, Rapido) paid at the shop is paid to the driver in
  cash and completes once the app is looking for a driver; its receipt says
  `kind` `ride` and `state` `requested`. A cancel after the order was placed
  at the shop still stops the purchase, but the intent comes back
  `cancelled` with `error.code` `may_be_placed`. No new status.
- SDKs: `kind` and `state` on `Receipt` (TypeScript, Python, Rust); both
  CLIs' `status` say a ride was requested.

### Removed (breaking)

Live purchases are paid only from the payer's own wallet, so the API's own
balance and its top-ups are gone, and so are stored cards.

- API: `GET /balance`, `POST /topups` and `GET /topups/{id}`, with the
  `Balance`, `Topup` and `CreateTopupParams` schemas. The MCP tools
  `get_balance`, `create_topup` and `get_topup` are gone too.
- API: the payment reason `insufficient_balance`; a purchase no longer waits
  on a balance. The other reasons stay.
- API: the secret kind `card`, the `cards_unavailable` error code and the
  `fund` use of a secret. A secret request can no longer ask for a card.
- SDKs: `balance.get` (TypeScript, Python), `balance` and `balance_for`
  (Rust), `topups.create` and `topups.get` (TypeScript, Python),
  `create_topup`, `create_topup_idempotent` and `get_topup` (Rust), the
  `Balance`, `BalanceParams`, `Topup`, `TopupStatus` and `CreateTopupParams`
  types, and `card` from `SecretKind`.
- CLI (npm and Rust): the `balance` and `topups` commands.

### Changed

- Customer `limits` (`per_intent`, `monthly`) count the cart's total of each
  purchase the payer's wallet paid this UTC month. They are checked before
  anything is paid, nothing the payer does in their wallet lifts them, and an
  intent over one fails with `limit_exceeded`.
- A receipt's `fee` is `0` for now.
- A wallet link's `status` may be `ended`: the purchase it was made for
  ended without being paid, so the link no longer opens.

## [0.2.0]

### Added

- CLI (npm and Rust, same commands and flags): `customers`
  (`create`, `update`, `get`, `list`, `portal`), `sessions` (`open`, `list`,
  `get`, `release`, `live`, `command`, `screenshot`), `profiles` (`list`,
  `create`, `get`, `forget`), `topups` (`create`, `get`), `sign-ins`
  (`create`, `get`) and `secrets` (`set`, `list`, `get`, `delete`,
  `request`). Secrets are write-only: no command prints a value, and
  `secrets set --stdin` takes the fields as JSON so they stay out of the
  process list. `balance --customer`. The Rust CLI gains `login` (it saves
  the key where the npm CLI does, and later commands read it), `run
  --context` and `list --status`; only the npm CLI has `mcp`.
- Versions: the three SDKs, the skill and the Claude plugin are all `0.2.0`
  (the registries were at `0.1.0` and `0.1.1`). `sdk/bump-versions.mjs
  --check` fails when any file disagrees, and the release workflow publishes
  one version to npm, PyPI and crates.io and then checks that all three
  report it.
- Country: `country` (ISO 3166-1 alpha-2, lowercase) and `end_user_ip` on
  `POST /intents`, `/sessions`, `/sign_ins` and the portal session call pick
  the country a browser goes out from. Objects carry `country` and
  `country_source` (`explicit`, `payer`, `end_user_ip`, `request_ip` or
  `default`). `422 country_unavailable` when no browser can go out from the
  country asked for. All three SDKs.
- Waiting: `Prefer: wait=N` on `POST /intents` and `GET /intents/{id}` holds
  the request up to 50 seconds until the intent finishes or pauses; the
  answer says `Preference-Applied`. TypeScript `{ wait }`, Python `wait=`,
  Rust `create_intent_waiting` and `get_intent_waiting`. The MCP tools
  `create_intent` and `answer_input` take `wait_seconds`.
- Structured questions: an `input_request` can offer `choices` (an `id`,
  `label`, `description` and `price_delta` each) or ask a form of `fields`
  (`text`, `email`, `phone`, `date`, `number` or `choice`), and says what a
  plain answer looks like in `expects`. Answer a form with `values`, a choice
  with its `id` as `value`. `input_ttl_seconds` on create sets how long a
  question may wait.
- Sensitive questions: `input_request.kind` is `sensitive` for an identity
  document, answered only on `input_request.url` (`422 answer_on_page`).
- Secrets: `POST/GET /secrets`, `GET/PUT/DELETE /secrets/{id}` and
  `GET /secrets/{id}/uses` store details a purchase may need without putting
  them in a prompt; they are write-only. `secrets` and `shops` on
  `POST /intents` name the ones a purchase may use. `POST
  /customers/{id}/secret_requests` and `GET /secret_requests/{id}` ask a
  customer to enter their own, and `secret_request.completed` ends one.
  The MCP tools `create_secret`, `list_secrets` and `delete_secret`.
- Sign-in links without an intent: `POST /sign_ins` and `GET /sign_ins/{id}`,
  ending in `sign_in.completed` or `sign_in.expired`.
- Live links: `POST /sessions/{id}/live_links` makes a 10 minute link to drive
  an open session, never to pay.
- Portal sessions: `POST /customers/{id}/portal_sessions` (`return_url`,
  `ttl_seconds`) makes a link to a customer's own page.
- Webhook endpoints: `/webhook_endpoints` (create, list, get, update, delete),
  `rotate_secret` (the old secret signs for 24 more hours; `verifyWebhook`
  and its twins accept a header with both), `test`, `deliveries` and
  `redeliver`; and `webhook_url` (with a `webhook_secret` in the create
  response) on `POST /intents` and `/sessions`.
- Limits: `RateLimit-Limit`, `RateLimit-Remaining` and `RateLimit-Reset` on
  responses and `429 rate_limit_exceeded` with `Retry-After`; the SDK errors
  carry `retryAfter`/`retry_after` and `rateLimit`/`rate_limit`. Customers take
  `limits` (`per_intent`, `monthly`) and the balance reports
  `per_intent_limit`.
- MCP: sessions (`Mcp-Session-Id`) and Server-Sent Events progress, tasks on
  `2025-11-25`, elicitation of plain questions, and OAuth sign-in for clients
  that support it.

- Details in `requires_input`: when the purchase needs something the
  request did not give, such as the email the shop wants for the order, a
  size or a colour, the agent asks the payer instead of failing.
  `input_request.kind` is then `detail` and `prompt` is the agent's own
  question (never about passwords, codes, PINs, card or bank details or ID
  numbers). The new `input_request.options` lists a detail's choices
  (empty otherwise), and an answer must then be one of them. A detail is
  answered with `value` on the same endpoint, at most 3 per intent, and a
  declined or expired one fails the intent with `needs_information`. All
  three SDKs: the `detail` kind and `options`, and the CLIs print the
  options.

- Agent sign-in: when the shop wants the payer signed in and asks for a
  phone number or email and then a one-time code, the agent signs them in
  itself. Each time it needs something only the payer knows, the intent
  waits in the new status `requires_input` with `input_request` (`id`,
  `kind`, `shop`, `prompt`, `channel`, `length`, `url`, `expires_at`,
  `attempts_left`), answered with `POST /intents/{id}/input` (`value`,
  `decline` or `done`; `409 input_not_waiting` once it has moved on), and
  `intent.requires_input` webhooks. `sign_in_agent: false` on create turns
  it off, and `sign_in_hint` passes the payer's phone or email so the agent
  need not ask. Passwords still go through the sign-in link, now the
  fallback. All three SDKs: the status, `input_request`, `sign_in_agent`,
  `sign_in_hint`, the answer (`intents.input` / `answer_intent_input`), and
  `wait` returning at `requires_input` by default (`returnOnInput` /
  `return_on_input`). The CLIs print the prompt and exit `5`, and
  `openintents input` answers.

- Sign-in links: when the shop wants the payer signed in, an intent waits in
  the new status `requires_login` with `sign_in: { url, shop, expires_at }`
  for the payer, instead of failing; `intent.requires_login` webhooks, and
  status reasons `login_required` and `signed_in`. `sign_in_link: false` on
  create keeps the old behaviour (fail with `login_required`). All three
  SDKs: the status, `sign_in`, `sign_in_link`, and `wait` returning at
  `requires_login` by default (`returnOnSignIn` / `return_on_sign_in`). The
  CLIs print the link and exit `4`.

- Spec: browser profiles (`/profiles`), browser sessions (`/sessions`, with
  commands, screenshots and release, and a `sessionEvent` webhook), event
  streams for intents and sessions, and `lease` on the error body for
  `profile_busy`. It also says how sessions behave: why they close
  (`data_limit` among the reasons), when the refs of a snapshot are
  forgotten, which text and keys a command may send, what a session refuses
  (`payment_not_allowed`), every failure code, that at most 2 screenshots
  of a session wait at once (`429 too_many_commands`), and that opening one
  answers `503 sessions_unavailable` while sessions are not open to the key.
- Spec: what the API already serves but the spec left out: customers
  (`/customers`, with the `external_id` filter), top-ups (`/topups`),
  `customer` on intents (and the `customer` filter on `GET /intents`), `held`
  and `customer` on the balance (and `?customer=`), and the server's limits
  and idempotency rules.
- Spec conventions: test and live keys share customers, intents, balances
  and profiles; unknown request fields are refused; new enum values may
  appear.
- SDKs (TypeScript, Python, Rust): customers (create, retrieve, update,
  list), top-ups (create, retrieve), a customer's balance, `customer` when
  creating and listing intents, and the balance's `held` and `customer`.
  Customer and top-up creates send an `Idempotency-Key`, like intents.
- SDKs: browser profiles (list, create, retrieve, forget); browser sessions
  (open, list, retrieve, release, run and retrieve commands, screenshots);
  intent and session events, as a page or as a live stream that resumes
  after a dropped connection (`Last-Event-ID`) and ends when the timeline is
  over (`204`); the error's `lease`; and session webhook events. Session
  creates, releases and commands send an `Idempotency-Key`, so a retry never
  runs a command twice.

### Changed

- MCP: the server takes an API key, sent as `Authorization: Bearer`. The
  Claude Code plugin sends `OPENINTENTS_API_KEY`, and the skill and README
  say how to connect other clients. The skill lists every MCP tool, with
  `answer_input` for what an intent asks the user and an `idempotency_key`
  on `create_intent` so a retry never buys twice.
- TypeScript and Python: `WebhookEvent` is a union of intent and session
  events; check `type` before reading `data`.
- Rust: `WebhookEvent.data` is a `WebhookData` (`Intent`, `Session` or
  `Other`, with `intent()` and `session()` helpers), `Error::Api` has a
  `lease`, and a stream that keeps closing empty ends with
  `Error::StreamClosed`.

## [0.1.0]

### Added

- OpenAPI spec for the v1 preview API (`spec/openapi.yaml`).
- TypeScript SDK and `openintents` CLI (`@tinyhumansai/openintents`).
- Python SDK with sync and async clients (`openintents`).
- Rust SDK with an optional CLI (`openintents`).
- Webhook signature verification in every SDK, with a shared test vector.
- Agent skill and Claude Code plugin.
