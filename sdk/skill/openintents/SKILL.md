---
name: openintents
description: Use when the user wants something bought, booked or ordered online (a coffee, a ride, a flight, groceries, an Amazon order). Sends a plain-language intent to the OpenIntents agentic API, which completes the purchase in a real browser and returns a receipt, or a payment link when a person needs to pay. Prefers the OpenIntents MCP tools; falls back to the `openintents` CLI.
license: GPL-3.0-or-later
compatibility: Needs network access to api.openintents.io and an OpenIntents account. Uses the OpenIntents MCP server (https://api.openintents.io/mcp) or the `openintents` CLI from `@tinyhumansai/openintents` (npm).
metadata:
  author: TinyHumans AI
  version: "0.2.0"
  package: "@tinyhumansai/openintents"
---

# OpenIntents

OpenIntents is an agentic API for transactions: you send an intent in plain
language and it completes the purchase end to end, then returns a structured
receipt. Inference, browser, compute and bandwidth are free. OpenIntents' fee
is zero for now, as shown on every receipt.

## Connect

Prefer the MCP tools (`create_intent`, `wait_for_intent`, `get_intent`,
`answer_input`, `list_intents`, `get_intent_events`, `cancel_intent`,
`create_secret`, `list_secrets`, `delete_secret`). The server takes the user's API
key (made at https://openintents.io/dashboard/api-keys), sent as
`Authorization: Bearer <key>`. If the tools aren't available, with the key in
`OPENINTENTS_API_KEY`:

- Claude Code: `claude mcp add --transport http openintents https://api.openintents.io/mcp --header "Authorization: Bearer $OPENINTENTS_API_KEY"`
- Codex: `codex mcp add openintents --url https://api.openintents.io/mcp --bearer-token-env-var OPENINTENTS_API_KEY`
- Otherwise use the CLI: `npm install -g @tinyhumansai/openintents`, then
  `openintents login` (or set `OPENINTENTS_API_KEY`).

## Send an intent

1. Turn the request into one specific sentence: what, where or from whom,
   when, and constraints ("flat white, oat milk, pickup 8:45 at the Blue Bottle
   on 5th").
2. Agree a budget with the user and always pass it as `max_amount` (cents), or
   `--max <dollars>` on the CLI.
3. Put structured details (addresses, names) in `context`, not the sentence.
   If the user has given you their phone number or email for signing in to
   the shop, you may pass it as `sign_in_hint` (`{ "phone": ... }` or
   `{ "email": ... }`) so the agent need not ask for it.
4. Create it and wait. Over MCP: `create_intent` with an `idempotency_key`
   of your choosing (send the same one if you retry, so it never buys twice),
   then `wait_for_intent`, again while the intent is still queued or running
   (each call waits up to 50 seconds). `create_intent` and `answer_input` take
   `wait_seconds` (0 to 50) to hold the call until the intent finishes or
   pauses, which saves a round trip. With the CLI:

   ```bash
   openintents run "flat white, oat milk, pickup 8:45 at the Blue Bottle on 5th" --max 10 --wait --json
   ```

5. Read the result:
   - `completed`: report merchant, items, total, fee and confirmation.
   - `requires_payment`: show the user what it is buying, explain
     `payment.reason`, and give them `payment.payment_url`:
     - `over_limit`: the purchase exceeds a spending limit; for a wallet,
       the user raises their limit on the payment page.
     - `requested`: a payment link was requested before proceeding.
     - `connect_wallet`: the user needs to connect their wallet or sign in
       to it again.
     - `add_money`: the user needs to add money to their wallet.
     - `identity_check`: the user needs to confirm their identity on the
       payment page. Never ask for identity documents in the chat.
     - `wallet_processing`: the wallet is processing the payment; the link
       shows its progress.

     For an unknown reason, say the purchase is waiting on a payment step
     and direct the user to the link without guessing why. Then wait again
     (`openintents status <id> --wait`); wallet purchases resume automatically
     once the step is done.
   - `requires_input`: the agent is signing the user in to the shop
     (`input_request.shop`) and needs something only they know. Ask them
     exactly `input_request.prompt`, naming the shop. Send their reply right
     away, as the request expires in 5 minutes:
     `openintents input <id> --input <input_request.id> --value "<reply>"`,
     or `--done` once they approved an `approval`. If they would rather not,
     send `--decline`. Over MCP it is `answer_input` with the intent's `id`,
     `input` (`input_request.id`) and one of `value`, `done` or `decline`.
     If they would rather type it on a page than in the chat, give them
     `input_request.url` (theirs only). Then wait again. If
     the intent later shows `requires_login`, give them the sign-in link
     instead.

     When `input_request.kind` is `detail`, the purchase needs something the
     request did not give, such as the email the shop wants for the order,
     a size or a colour. Ask it the same way: exactly
     `input_request.prompt`, offering `input_request.options` as the choices
     when there are any. Send the user's reply with `--value` (one of the
     options, when there are any). The agent never asks for a secret this
     way: if a prompt ever asks for a password, a code, a PIN, card or bank
     details or an ID number, don't ask the user; send `--decline`.

     A detail can also offer `input_request.choices` (each with an `id`, a
     `label`, maybe a `description` and a `price_delta` in minor units): show
     the labels and prices, and answer with the `id` of the one the user picks
     as `value`. Or it can ask a form of `input_request.fields` (each with a
     `name`, `label`, `type` and whether it is `required`; a `choice` field
     lists `options`): ask for each field and answer all at once with
     `values`, an object keyed by field `name`. `input_request.expects` says
     what a plain answer looks like (`text`, `email`, `phone` or `choice`).
     The question may stay open for a long while if the user is away, so the
     5 minute hurry applies to sign-in questions only.

     When `input_request.kind` is `sensitive`, the shop wants an identity
     document (a passport for a flight). Never ask for it in the chat and
     never answer it yourself: give the user `input_request.url`, where they
     type it themselves, and wait again. Only a decline (`--decline`) is
     accepted by the answer endpoint (`422 answer_on_page` otherwise).

     When `input_request.kind` is `payment` (the intent was created with
     `pay_at_shop`), the user pays the shop themselves: show them
     `input_request.image_url`, a picture of the shop's payment code to scan
     with a UPI app, and answer `--done` once they say they paid (or
     `--decline`). It expires after 5 minutes.
   - `requires_login`: the shop wants the user signed in to their account
     there. Give the user `sign_in.url` (theirs only: it works on the first
     device that opens it), tell them it works for 30 minutes, and wait
     again once they say they're done.
   - `failed`: report `error.message`; ask before retrying. On
     `429 rate_limit_exceeded`, wait `Retry-After` seconds before the next
     call.

CLI exit codes: 0 completed, 1 failed or error, 2 needs payment, 3 cancelled,
4 needs the user to sign in, 5 needs an answer from the user.

## Country, secrets and sign-in links

- The browser goes out from the user's country. Pass `country` (two letters,
  lowercase, such as `fr`) when you know where the purchase must look like it
  comes from, or `end_user_ip` (the user's own address) to have it inferred.
  The intent says which was used in `country` and `country_source`.
- For details the purchase needs and that you would not put in a prompt
  (an address, contact details, a loyalty number, a shop password),
  store them once with `create_secret` (a `kind`, a `label`, and `fields`)
  and name them on the intent with `secrets: ["sec_..."]` and the `shops` they
  may be filled on. A secret is write-only: you only ever get a label, a mask
  and public fields back, never its values. `list_secrets` and
  `delete_secret` manage them. To ask the user to enter theirs themselves,
  use the REST API's secret requests (`POST /customers/{id}/secret_requests`)
  and send them the `url`.
- To sign a user in to a shop before any purchase, make a sign-in link
  (`POST /sign_ins` with a `shop_url`) and send them its `url`; the end is a
  `sign_in.completed` or `sign_in.expired` webhook.

## Rules

- Only make purchases the user asked for, within the budget they gave.
- Never pay a payment link yourself, and never ask for, store or invent card
  numbers. Payment happens on the hosted payment page or from the balance.
- Never open a sign-in link, an `input_request.url`, a secret request's `url`
  or a live link yourself, and never log a secret's values. Never ask the
  user for a password, a card PIN, a UPI PIN or a bank OTP: ask only what
  `input_request.prompt` says, pass their reply on as it is, and don't
  store or repeat it.
- Use a test key (`oi_test_...`) when the user is only trying things out: test
  intents run end to end but never charge.
- Cancel (`openintents cancel <id>`) anything the user changes their mind
  about before it pays; cancelled intents are free.

Docs: https://openintents.io/docs. Agent-readable index: https://openintents.io/llms.txt.
