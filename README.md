<img width="1200" height="630" alt="image" src="https://openintents.io/opengraph-image?4ea0166ce5814a38" />

# OpenIntents

**The agentic API to buy anything.** Send one intent in plain language, like
_"book me a trip to Bali"_ or _"get my lunch sorted: a bacon cheeseburger"_.
The OpenIntents API does the agentic work in a real browser, gets it paid
from the payer's own wallet, and returns a structured receipt.
Inference, browser, compute and bandwidth are free.

This repository holds the open-source client side: SDKs for TypeScript,
Python and Rust, the `openintents` CLI, the OpenAPI spec, an agent skill and a
Claude Code plugin.

> **Preview.** The API is being built in the open. Shapes may change before
> launch; see [`spec/openapi.yaml`](spec/openapi.yaml) and the
> [docs](https://openintents.io/docs).

## Documentation

The guides live on the website, not in this repository:
<https://openintents.io/docs>. We point at them instead of keeping copies
here because two copies of a guide drift apart; the one thing this repository
owns is the contract, [`spec/openapi.yaml`](spec/openapi.yaml), and every SDK
and CLI README follows it. Each guide is also served as raw Markdown at
`https://openintents.io/docs/raw/<page>`, and indexed for agents at
<https://openintents.io/llms.txt>.

| To do this | Read |
| --- | --- |
| Send an intent and follow it to a receipt | [Intents](https://openintents.io/docs/intents) and [Quickstart](https://openintents.io/docs/quickstart) |
| Answer what the agent asks (a size, a code), or sign a payer in to a shop | [Intents](https://openintents.io/docs/intents) and the "Answer what the agent asked" and "Sign-in links" sections of the [REST API](https://openintents.io/docs/api) |
| Use OpenIntents from Claude, Cursor, Codex or any MCP client | [MCP server](https://openintents.io/docs/mcp) |
| Get told when an intent changes | [Webhooks](https://openintents.io/docs/webhooks) |
| Act for your own customers (payers), with a portal for them | The "Customers" and "Portal sessions" sections of the [REST API](https://openintents.io/docs/api) |
| Store an address, phone number or shop password once, write-only | [Secrets](https://openintents.io/docs/vault) |
| Keep a payer's logins between intents, or drive their browser | [Browser profiles](https://openintents.io/docs/profiles) and [Browser sessions](https://openintents.io/docs/sessions) |
| Follow a timeline live | [Events & live streams](https://openintents.io/docs/events) |
| Pay from the payer's own wallet, and what to do when a purchase waits | [Wallets & payments](https://openintents.io/docs/wallets) |
| Have a customer set up the wallet they pay from, before their first purchase | [Wallet links](https://openintents.io/docs/wallets#wallet-links) |
| Use the command line | [CLI](https://openintents.io/docs/cli) |

If a guide and the spec ever disagree, the spec wins; please open an issue.

## Install

| | |
| --- | --- |
| TypeScript / JavaScript | `npm install @tinyhumansai/openintents` |
| Python | `pip install openintents` |
| Rust | `cargo add openintents` |
| CLI | `npm install -g @tinyhumansai/openintents` |
| Claude Code | `/plugin marketplace add tinyhumansai/openintents` |
| Any MCP client | `https://api.openintents.io/mcp`, with your API key as `Authorization: Bearer` |
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
// "requires_payment" → send the payer intent.payment.payment_url
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
3. **Pay from the payer's wallet.** Live purchases are paid from the payer's
   own wallet, within the limits they set there. When the wallet needs a step
   first (connecting it, adding money, an identity check), the intent returns
   `requires_payment` with a `payment_url` for the payer and carries on by
   itself once they are done.
4. **Get the receipt.** Merchant, items, total, fee and confirmation, as JSON.
   Webhooks fire as the intent moves (`intent.completed`,
   `intent.requires_payment`, `intent.requires_input`, `intent.failed` and
   the rest of the list in the spec).

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
