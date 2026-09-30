# Examples

Each example sends the same intent, waits, and prints the receipt or the
payment link. Set `OPENINTENTS_API_KEY` to a test key (`oi_test_...`) first:
test intents run end to end but never charge.

| Language | File | Run |
| --- | --- | --- |
| TypeScript | [`buy-coffee.ts`](buy-coffee.ts) | `npx tsx sdk/examples/buy-coffee.ts` |
| Python | [`buy_coffee.py`](buy_coffee.py) | `uv run --with openintents sdk/examples/buy_coffee.py` |
| Rust | [`../rust/examples/buy_coffee.rs`](../rust/examples/buy_coffee.rs) | `cd sdk/rust && cargo run --example buy_coffee` |
