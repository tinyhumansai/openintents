import { readFileSync } from "node:fs";

import { describe, expect, it, vi } from "vitest";

import { describeIntent } from "../src/cli-format.js";
import { OpenIntents, type Intent } from "../src/index.js";

const example = (name: string): Intent =>
  JSON.parse(readFileSync(new URL(`../../../spec/examples/${name}`, import.meta.url), "utf8")) as Intent;

function mockFetch(...bodies: Array<{ status?: number; body: unknown }>) {
  const calls: Array<{ url: string; init: RequestInit }> = [];
  const fn = vi.fn(async (url: URL | RequestInfo, init?: RequestInit) => {
    calls.push({ url: String(url), init: init ?? {} });
    const r = bodies.shift()!;
    return new Response(JSON.stringify(r.body), { status: r.status ?? 200 });
  });
  return { fetch: fn as unknown as typeof fetch, calls };
}

const make = (f: typeof fetch) => new OpenIntents({ apiKey: "oi_test_x", baseUrl: "https://api.test/v1", fetch: f, maxRetries: 0 });

describe("order review", () => {
  it("sends review_before_order on create and reads the review question", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: example("intent-requires-review.json") });
    const got = await make(fetch).intents.create({ intent: "snacks", pay_at_shop: true, review_before_order: true });
    expect(JSON.parse(String(calls[0]!.init.body))).toEqual({ intent: "snacks", pay_at_shop: true, review_before_order: true });
    expect(got.status).toBe("requires_input");
    expect(got.review_before_order).toBe(true);
    const r = got.input_request!;
    expect(r.kind).toBe("review");
    expect(r.review).toEqual({
      merchant: "blinkit.com",
      items: [
        { name: "Protein bar, sugar free", quantity: 2, amount: 10000 },
        { name: "Roasted chana", quantity: 1, amount: 4500 },
      ],
      fees: [
        { label: "Delivery fee", amount: 2500 },
        { label: "Cash on delivery fee", amount: 1000 },
      ],
      total: 18000,
      currency: "inr",
      deliver_to: "Home, 12 MG Road, Bengaluru 560001",
      eta: "12 minutes",
      payment_method: "cash_on_delivery",
    });
  });

  it("answers a review with done, a change as a value, or a decline", async () => {
    const running = { body: { ...example("intent-requires-review.json"), status: "running", input_request: null } };
    const { fetch, calls } = mockFetch(running, running, { body: example("intent-review-declined.json") });
    const oi = make(fetch);
    await oi.intents.input("int_review_1", { input: "inp_review_1", done: true });
    await oi.intents.input("int_review_1", { input: "inp_review_1", value: "remove the chana" });
    const declined = await oi.intents.input("int_review_1", { input: "inp_review_1", decline: true });
    expect(calls.map((c) => JSON.parse(String(c.init.body)))).toEqual([
      { input: "inp_review_1", done: true },
      { input: "inp_review_1", value: "remove the chana" },
      { input: "inp_review_1", decline: true },
    ]);
    expect(calls.every((c) => c.url === "https://api.test/v1/intents/int_review_1/input")).toBe(true);
    expect([declined.status, declined.error?.code]).toEqual(["failed", "order_declined"]);
  });

  it("reads the delivery address the payer approved on the receipt", async () => {
    const { fetch } = mockFetch({ body: example("intent-completed-reviewed.json") });
    const got = await make(fetch).intents.get("int_review_1");
    expect(got.result?.deliver_to).toBe("Home, 12 MG Road, Bengaluru 560001");
    expect(got.result?.paid_with).toBe("cash_on_delivery");
  });

  it("prints the order to review and how to answer it", () => {
    const out = describeIntent(example("intent-requires-review.json"));
    expect(out).toContain("order to review at blinkit.com:");
    expect(out).toContain("2 x Protein bar, sugar free  ₹100.00");
    expect(out).toContain("Cash on delivery fee  ₹10.00");
    expect(out).toContain("total  ₹180.00");
    expect(out).toContain("deliver to: Home, 12 MG Road, Bengaluru 560001");
    expect(out).toContain("arrives: 12 minutes");
    expect(out).toContain("pay by: cash on delivery");
    expect(out).toContain("--input inp_review_1 --done to place it (or --value <change>, or --decline)");
  });

  it("offers only a decline for a review whose order is missing", () => {
    const base = example("intent-requires-review.json");
    for (const review of [undefined, { ...base.input_request!.review!, items: [] }]) {
      const out = describeIntent({ ...base, input_request: { ...base.input_request!, review } });
      expect(out).toContain("--input inp_review_1 --decline (the order to review is missing, so it cannot be approved)");
      expect(out).not.toContain("--done");
      expect(out).not.toContain("order to review at");
    }
  });

  it("prints the receipt's delivery address", () => {
    expect(describeIntent(example("intent-completed-reviewed.json"))).toContain("delivered to: Home, 12 MG Road, Bengaluru 560001");
  });

  it("still reads and prints a question of a kind it does not know", async () => {
    const future = example("intent-requires-review.json");
    const body = { ...future, input_request: { ...future.input_request!, kind: "something_new", review: undefined } };
    const { fetch } = mockFetch({ body });
    const got = await make(fetch).intents.get("int_review_1");
    expect(got.input_request?.kind).toBe("something_new");
    expect(describeIntent(got)).toContain("--input inp_review_1 --value <answer> (or --decline)");
  });
});
