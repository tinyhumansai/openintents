# @tinyhumansai/openintents

TypeScript SDK and CLI for [OpenIntents](https://openintents.io), the agentic
API to buy anything. Send an intent in plain language; the API does the
agentic work in a real browser, gets it paid, and resolves to a receipt.
Inference, browser, compute and bandwidth are free.

> Preview: the API is still being built, so it may change before launch.

```bash
npm install @tinyhumansai/openintents
```

Zero runtime dependencies. Works in Node 20.3+, Bun, Deno and edge runtimes
(uses `fetch` and Web Crypto).

## SDK

```ts
import { OpenIntents } from "@tinyhumansai/openintents";

const oi = new OpenIntents(); // reads OPENINTENTS_API_KEY

const created = await oi.intents.create({
  intent: "flat white, oat milk, pickup 8:45 at the Blue Bottle on 5th",
  max_amount: 1000, // cents: never spend more than $10.00
});

const intent = await oi.intents.wait(created.id); // final, or needs payment

if (intent.status === "requires_payment") {
  console.log("Send the payer:", intent.payment?.payment_url); // a step in their wallet
} else if (intent.status === "completed") {
  console.log(intent.result?.merchant, intent.result?.total);
}
```

Also: `oi.intents.get(id)`, `oi.intents.list({ status, customer, limit })`,
`for await (const i of oi.intents.listAll())`, `oi.intents.cancel(id)`,
`oi.intents.input(id, { input, value })` (see below),
`oi.balance.get({ customer })`, `oi.customers.create/get/update/list/listAll`
and `oi.topups.create/get`. Pass `customer` to `intents.create` to buy for one
of your customers. Errors throw `OpenIntentsError` (`status`, `type`, `code`,
`param`, and `lease` with `profile_busy`). Every create (but a profile's),
`cancel`, `input`, `release` and `runCommand` sends an `Idempotency-Key`
automatically; 429s, 5xx and network errors are retried with backoff.

### Signing in to a shop

When a shop wants the payer signed in and asks for a phone number or email
and then a code, the agent signs them in itself. Each time it needs
something only the payer knows, `wait` returns the intent in
`requires_input`. Ask the payer `input_request.prompt` and answer within 5
minutes; the agent's model never sees the answer:

```ts
if (intent.status === "requires_input" && intent.input_request) {
  const answer = await askThePayer(intent.input_request.prompt); // your chat, say
  await oi.intents.input(intent.id, { input: intent.input_request.id, value: answer });
  // or { input, decline: true }, or { input, done: true } for an approval
}
```

Or send the payer `input_request.url` to type it there themselves. Pass
`sign_in_hint: { phone }` or `{ email }` to `create` to skip the first
question. Passwords are never asked this way: those shops send a sign-in
link instead (`requires_login`, `sign_in.url`).

The same status asks for anything else the purchase needs that the request
did not give, such as the email the shop wants for the order or a size:
`input_request.kind` is then `detail`, `prompt` is the agent's own question,
and `options` lists the choices when there are any (answer with one of
them). Answer it the same way, with `value`; the answer goes back to the
agent. An intent asks at most 3 details, and never for a secret.

### Live events

Follow an intent as it runs. The stream resumes by itself after a dropped
connection, and ends once the intent is over:

```ts
for await (const event of oi.intents.streamEvents(created.id)) {
  console.log(event.type, event.data); // status, step, checkout
}
```

`oi.intents.listEvents(id, { limit, cursor })` pages the same timeline.

### Browser sessions

A session opens a payer's browser profile without an intent: to sign them in
to a shop once, so later intents find them signed in. A session never pays
or places an order.

```ts
const session = await oi.sessions.create({ customer: "cus_4b81d0", start_url: "https://shop.example/login" });
for await (const event of oi.sessions.streamEvents(session.id)) {
  if (event.type === "status" && event.data.status === "open") break;
}
const page = await oi.sessions.runCommand(session.id, { type: "snapshot" }); // page.snapshot has the refs
await oi.sessions.runCommand(session.id, { type: "act", ref: "e12", action: "type", text: "ada@example.com" });
const jpeg = await oi.sessions.screenshot(session.id); // Uint8Array
await oi.sessions.release(session.id); // the profile keeps the sign-in
```

Also `oi.sessions.list/listAll/get/getCommand/listEvents` and
`oi.profiles.list/listAll/create/get/forget`.

### Country, waiting and forms

`country` (ISO 3166-1 alpha-2, lowercase) or `end_user_ip` on `intents.create`,
`sessions.create`, `signIns.create` and `customers.createPortalSession` picks
the country the browser goes out from; the object says which in `country` and
`country_source`. Pass `{ wait: 30 }` as the second argument of
`intents.create` or `intents.get` to have the API hold the request up to 50
seconds until the intent finishes or pauses (`Prefer: wait`).

A question in `input_request` can offer `choices` (answer with one's `id`) or
ask a form of `fields` (answer with `{ input, values: { name: value } }`);
`expects` says what a plain answer looks like. A `sensitive` question (an
identity document) is answered only on `input_request.url`.

### Secrets, sign-ins and live links

```ts
const card = await oi.secrets.create({ kind: "card", label: "Work Visa", fields: { number: "4242424242424242", /* … */ }, shops: ["airfrance.com"] });
await oi.intents.create({ intent: "book the 7am flight", secrets: [card.id], shops: ["airfrance.com"] });

const ask = await oi.customers.createSecretRequest("cus_4b81d0", { kinds: ["identity_document"] }); // send ask.url
const signIn = await oi.signIns.create({ shop_url: "https://www.amazon.com/", customer: "cus_4b81d0" }); // send signIn.url
const portal = await oi.customers.createPortalSession("cus_4b81d0", { ttl_seconds: 1800 }); // send portal.url
const live = await oi.sessions.createLiveLink(session.id); // live.url drives the session, never to pay
```

Secrets are write-only: they come back as a label, a mask and public fields,
never their values (`oi.secrets.list/get/replace/delete/uses`).

### Webhook endpoints

```ts
const endpoint = await oi.webhookEndpoints.create({ url: "https://example.com/hooks", events: ["intent.completed", "sign_in.completed"] });
// endpoint.secret is shown here only. Also: list/listAll/get/update/delete,
await oi.webhookEndpoints.rotateSecret(endpoint.id); // the old secret signs for 24 more hours
await oi.webhookEndpoints.test(endpoint.id);          // sends webhook_endpoint.test to it alone
await oi.webhookEndpoints.listDeliveries(endpoint.id);
```

A 429 `OpenIntentsError` carries `retryAfter` and `rateLimit` (`limit`,
`remaining`, `reset`, from the `RateLimit-*` headers).

### Webhooks

```ts
import { verifyWebhook } from "@tinyhumansai/openintents";

// Use the raw body, before JSON parsing.
const event = await verifyWebhook(rawBody, req.headers.get("OpenIntents-Signature"), process.env.WEBHOOK_SECRET!);
if (event.type === "intent.requires_payment") notify(event.data.payment?.payment_url);
```

Session events (`session.opened`, `session.closed`, `session.failed`) carry
a session in `data`: check `event.type` first.

## CLI

```bash
npm install -g @tinyhumansai/openintents
openintents login
openintents run "UberX to SFO, leaving 7am" --max 60 --wait
openintents status <id> --wait --json
openintents pay <id>      # open the payment link (a step in the wallet)
openintents input <id> --input <input id> --value <code>
openintents balance [--customer <id>]
openintents customers create --external-id u-7 --name Ada --per-intent 50
openintents customers portal <id>          # a link to the customer's own page
openintents sessions open --customer <id> --start-url https://shop.example/
openintents sessions command <id> snapshot # or navigate, read, click, type, select, press
openintents sessions live <id>             # a 10 minute link to drive the browser
openintents profiles list --customer <id>
openintents topups create 25 --customer <id>
openintents sign-ins create https://www.amazon.com/ --customer <id>
echo '{"number":"4242424242424242"}' | openintents secrets set --kind card --label "Work Visa" --stdin
openintents secrets list                   # handles and masks, never values
openintents mcp           # stdio MCP server, see below
```

Exit codes: `0` completed, `1` failed or error, `2` needs payment, `3`
cancelled, `4` needs the payer to sign in to the shop (`sign_in.url`), `5`
needs an answer from the payer (`input_request.prompt`).

The same commands and flags exist in the Rust CLI (`cargo install openintents
--features cli`), except `mcp`. `customers`, `sessions`, `profiles`, `topups`,
`sign-ins` and `secrets` each take `--json` like the rest; `openintents --help`
lists every flag. Stored secrets are write-only: no command prints a value,
and `secrets set --stdin` takes the fields as a JSON object so they stay out
of the process list and your shell history.

### `openintents mcp`: MCP over stdio

A local stdio MCP server for clients that cannot speak the hosted
streamable-HTTP endpoint (`https://api.openintents.io/mcp`). It relays each
JSON-RPC message to that endpoint with your key, and passes the answers back
as they arrive, so progress notifications and elicitation requests reach the
client while a call is still running. The client's reply to an elicitation is
relayed back the same way.

```json
{
  "mcpServers": {
    "openintents": {
      "command": "npx",
      "args": ["-y", "@tinyhumansai/openintents", "mcp"],
      "env": { "OPENINTENTS_API_KEY": "oi_test_..." }
    }
  }
}
```

The key is `OPENINTENTS_API_KEY` or the one saved by `openintents login`. The
endpoint defaults to the hosted one; `--url` or `OPENINTENTS_MCP_URL` points
elsewhere, and `OPENINTENTS_BASE_URL` (the REST base, ending in `/v1`) moves
it along. Stdout carries only the protocol; problems go to stderr.

The same relay is available to programs as `runMcpBridge({ url, apiKey })`.

## Development

```bash
pnpm install
pnpm --filter @tinyhumansai/openintents lint
pnpm --filter @tinyhumansai/openintents test
pnpm --filter @tinyhumansai/openintents build
```

Docs: <https://openintents.io/docs>. License: GPL-3.0-or-later.
