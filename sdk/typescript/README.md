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
  console.log("Pay here:", intent.payment?.payment_url);
} else if (intent.status === "completed") {
  console.log(intent.result?.merchant, intent.result?.total);
}
```

Also: `oi.intents.get(id)`, `oi.intents.list({ status, limit })`,
`for await (const i of oi.intents.listAll())`, `oi.intents.cancel(id)` and
`oi.balance.get()`. Errors throw `OpenIntentsError` (`status`, `type`, `code`,
`param`). `create` and `cancel` send an `Idempotency-Key` automatically; 429s,
5xx and network errors are retried with backoff.

### Webhooks

```ts
import { verifyWebhook } from "@tinyhumansai/openintents";

// Use the raw body, before JSON parsing.
const event = await verifyWebhook(rawBody, req.headers.get("OpenIntents-Signature"), process.env.WEBHOOK_SECRET!);
if (event.type === "intent.requires_payment") notify(event.data.payment?.payment_url);
```

## CLI

```bash
npm install -g @tinyhumansai/openintents
openintents login
openintents run "UberX to SFO, leaving 7am" --max 60 --wait
openintents status <id> --wait --json
openintents pay <id>      # open the payment link
openintents balance
```

Exit codes: `0` completed, `1` failed or error, `2` needs payment, `3`
cancelled.

## Development

```bash
pnpm install
pnpm --filter @tinyhumansai/openintents lint
pnpm --filter @tinyhumansai/openintents test
pnpm --filter @tinyhumansai/openintents build
```

Docs: <https://openintents.io/docs>. License: GPL-3.0-or-later.
