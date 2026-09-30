---
name: openintents
description: Use when the user wants something bought, booked or ordered online (a coffee, a ride, a flight, groceries, an Amazon order). Sends a plain-language intent to OpenIntents, which gets it bought in a real browser and returns a receipt, or a payment link when a person needs to pay. Prefers the OpenIntents MCP tools; falls back to the `openintents` CLI.
license: GPL-3.0-or-later
compatibility: Needs network access to api.openintents.io and an OpenIntents account. Uses the OpenIntents MCP server (https://api.openintents.io/mcp) or the `openintents` CLI from `@tinyhumansai/openintents` (npm).
metadata:
  author: TinyHumans AI
  version: "0.1.1"
  package: "@tinyhumansai/openintents"
---

# OpenIntents

OpenIntents lets your agent buy anything: you send an intent in plain language
and it gets bought end to end, then you get a structured receipt. Inference, browser, compute and bandwidth are free; completed
transactions carry a 5% fee that is shown on every receipt.

## Connect

Prefer the MCP tools (`create_intent`, `get_intent`, `wait_for_intent`,
`list_intents`, `cancel_intent`, `get_balance`). If they aren't available:

- Claude Code: `claude mcp add --transport http openintents https://api.openintents.io/mcp`
- Codex: `codex mcp add openintents --url https://api.openintents.io/mcp`
- Otherwise use the CLI: `npm install -g @tinyhumansai/openintents`, then
  `openintents login` (or set `OPENINTENTS_API_KEY`).

## Send an intent

1. Turn the request into one specific sentence: what, where or from whom,
   when, and constraints ("flat white, oat milk, pickup 8:45 at the Blue Bottle
   on 5th").
2. Agree a budget with the user and always pass it as `max_amount` (cents), or
   `--max <dollars>` on the CLI.
3. Put structured details (addresses, names) in `context`, not the sentence.
4. Create it and wait:

   ```bash
   openintents run "flat white, oat milk, pickup 8:45 at the Blue Bottle on 5th" --max 10 --wait --json
   ```

5. Read the result:
   - `completed`: report merchant, items, total, fee and confirmation.
   - `requires_payment`: the balance can't cover it. Show the user what it is
     buying and give them `payment.payment_url`. Then wait again
     (`openintents status <id> --wait`).
   - `failed`: report `error.message`; ask before retrying.

CLI exit codes: 0 completed, 1 failed or error, 2 needs payment, 3 cancelled.

## Rules

- Only make purchases the user asked for, within the budget they gave.
- Never pay a payment link yourself, and never ask for, store or invent card
  numbers. Payment happens on the hosted payment page or from the balance.
- Use a test key (`oi_test_...`) when the user is only trying things out: test
  intents run end to end but never charge.
- Cancel (`openintents cancel <id>`) anything the user changes their mind
  about before it pays; cancelled intents are free.

Docs: https://openintents.io/docs. Agent-readable index: https://openintents.io/llms.txt.
