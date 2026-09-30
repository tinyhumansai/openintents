# OpenIntents

**The agentic API to buy anything.** Send one intent in plain language, like
_"book me a trip to Bali"_ or _"get my lunch sorted: a bacon cheeseburger"_.
The OpenIntents API does the agentic work in a real browser, gets it paid
(from your agent balance, or with a payment link) and returns a structured
receipt. Inference, browser, compute and bandwidth are free.

This repository holds the open-source client side: SDKs for TypeScript,
Python and Rust, the `openintents` CLI, the OpenAPI spec, an agent skill and a
Claude Code plugin.

> **Preview.** The API is being built in the open. Shapes may change before
> launch; see [`spec/openapi.yaml`](spec/openapi.yaml) and the
> [docs](https://openintents.io/docs).

## Install

| | |
| --- | --- |
| TypeScript / JavaScript | `npm install @tinyhumansai/openintents` |
| Python | `pip install openintents` |
| Rust | `cargo add openintents` |
| CLI | `npm install -g @tinyhumansai/openintents` |
| Claude Code | `/plugin marketplace add tinyhumansai/openintents` |
| Any MCP client | `https://api.openintents.io/mcp` |
| Any agent | "Read https://openintents.io/llms.txt and follow the instructions to set up OpenIntents." |

## Quickstart

Get an API key at <https://openintents.io/dashboard/api-keys> (start with an
`oi_test_` key: test intents run end to end but never charge).

```ts
import { OpenIntents } from "@tinyhumansai/openintents";

const oi = new OpenIntents(); // OPENINTENTS_API_KEY
const created = await oi.intents.create({ intent: "flat white, oat milk, pickup 8:45", max_amount: 1000 });
const intent = await oi.intents.wait(created.id);
// "completed" → intent.result is the receipt
// "requires_payment" → send someone intent.payment.payment_url
```

```python
from openintents import OpenIntents

oi = OpenIntents()
intent = oi.intents.wait(oi.intents.create("book an Uber to SFO at 7am", max_amount=6000)["id"])
```

```bash
openintents run "restock the fridge for the week" --max 120 --wait
```

## How an intent works

1. **Call the API with an intent.** Plain language, a spending cap in cents,
   optional structured `context`.
2. **The API does the agentic work.** Behind the endpoint, a real browser
   finds the merchant, picks the item and gets to checkout.
3. **Pay with your balance or a link.** A topped-up agent balance pays within
   your limits; otherwise the intent returns `requires_payment` with a
   `payment_url` for a person to pay.
4. **Get the receipt.** Merchant, items, total, fee and confirmation, as JSON.
   Webhooks fire on every status change.

## Repository layout

| Path | What |
| --- | --- |
| [`spec/openapi.yaml`](spec/openapi.yaml) | The v1 API: the contract every SDK follows |
| [`sdk/typescript/`](sdk/typescript) | `@tinyhumansai/openintents`: SDK + CLI |
| [`sdk/python/`](sdk/python) | `openintents` on PyPI: sync and async clients |
| [`sdk/rust/`](sdk/rust) | `openintents` on crates.io |
| [`sdk/skill/openintents/`](sdk/skill/openintents) | Agent skill (also [`SKILL.md`](SKILL.md)) |
| [`sdk/plugin-claude/`](sdk/plugin-claude) | Claude Code plugin |
| [`sdk/examples/`](sdk/examples) | One runnable example per language |

## Contributing

Issues and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md)
and [AGENTS.md](AGENTS.md) (which coding agents read too). Report security
issues privately: [SECURITY.md](SECURITY.md).

## License

[GPL-3.0-or-later](LICENSE).
