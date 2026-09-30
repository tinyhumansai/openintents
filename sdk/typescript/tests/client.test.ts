import { describe, expect, it, vi } from "vitest";

import { OpenIntents, OpenIntentsError, WaitTimeoutError, type Intent } from "../src/index.js";

const intent = (over: Partial<Intent> = {}): Intent => ({
  id: "int_1",
  object: "intent",
  status: "queued",
  intent: "flat white",
  test: true,
  created_at: "2026-10-01T08:30:00Z",
  ...over,
});

function mockFetch(...responses: Array<{ status?: number; body: unknown; headers?: Record<string, string> }>) {
  const calls: Array<{ url: string; init: RequestInit }> = [];
  const fn = vi.fn(async (url: URL | RequestInfo, init?: RequestInit) => {
    calls.push({ url: String(url), init: init ?? {} });
    const r = responses.shift() ?? responses.at(-1)!;
    return new Response(JSON.stringify(r.body), { status: r.status ?? 200, headers: r.headers });
  });
  return { fetch: fn as unknown as typeof fetch, calls };
}

const make = (f: typeof fetch, extra = {}) =>
  new OpenIntents({ apiKey: "oi_test_x", baseUrl: "https://api.test/v1", fetch: f, maxRetries: 0, ...extra });

describe("OpenIntents client", () => {
  it("requires an API key", () => {
    const prev = process.env.OPENINTENTS_API_KEY;
    delete process.env.OPENINTENTS_API_KEY;
    expect(() => new OpenIntents()).toThrow(/API key/);
    process.env.OPENINTENTS_API_KEY = prev;
  });

  it("creates an intent with auth, JSON body and an idempotency key", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: intent() });
    const created = await make(fetch).intents.create({ intent: "flat white", max_amount: 1000 });
    expect(created.id).toBe("int_1");
    const { url, init } = calls[0]!;
    expect(url).toBe("https://api.test/v1/intents");
    expect(init.method).toBe("POST");
    const headers = init.headers as Record<string, string>;
    expect(headers.Authorization).toBe("Bearer oi_test_x");
    expect(headers["Idempotency-Key"]).toMatch(/^[0-9a-f-]{36}$/);
    expect(JSON.parse(init.body as string)).toEqual({ intent: "flat white", max_amount: 1000 });
  });

  it("passes list filters as query parameters", async () => {
    const { fetch, calls } = mockFetch({ body: { object: "list", data: [intent()], next_cursor: null } });
    await make(fetch).intents.list({ status: "completed", limit: 5 });
    expect(calls[0]!.url).toBe("https://api.test/v1/intents?status=completed&limit=5");
  });

  it("follows cursors in listAll", async () => {
    const { fetch } = mockFetch(
      { body: { object: "list", data: [intent({ id: "a" })], next_cursor: "a" } },
      { body: { object: "list", data: [intent({ id: "b" })], next_cursor: null } },
    );
    const ids: string[] = [];
    for await (const i of make(fetch).intents.listAll()) ids.push(i.id);
    expect(ids).toEqual(["a", "b"]);
  });

  it("maps API errors to OpenIntentsError", async () => {
    const { fetch } = mockFetch({
      status: 400,
      body: { error: { type: "invalid_request", code: "max_amount_too_low", message: "too low", param: "max_amount" } },
    });
    const err = await make(fetch).intents.create({ intent: "x" }).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(OpenIntentsError);
    expect(err).toMatchObject({ status: 400, type: "invalid_request", code: "max_amount_too_low", param: "max_amount" });
  });

  it("retries 5xx responses", async () => {
    const { fetch, calls } = mockFetch({ status: 503, body: {}, headers: { "retry-after": "0.01" } }, { body: intent() });
    const got = await make(fetch, { maxRetries: 1 }).intents.get("int_1");
    expect(got.id).toBe("int_1");
    expect(calls).toHaveLength(2);
  });

  it("wait returns at requires_payment by default", async () => {
    const { fetch } = mockFetch(
      { body: intent({ status: "running" }) },
      {
        body: intent({
          status: "requires_payment",
          payment: { reason: "insufficient_balance", payment_url: "https://pay/p", total: 550, fee: 28, currency: "usd", expires_at: "x" },
        }),
      },
    );
    const got = await make(fetch).intents.wait("int_1", { intervalMs: 1 });
    expect(got.status).toBe("requires_payment");
    expect(got.payment?.payment_url).toBe("https://pay/p");
  });

  it("wait keeps polling past payment when asked", async () => {
    const { fetch } = mockFetch(
      { body: intent({ status: "requires_payment" }) },
      { body: intent({ status: "completed" }) },
    );
    const got = await make(fetch).intents.wait("int_1", { intervalMs: 1, returnOnPayment: false });
    expect(got.status).toBe("completed");
  });

  it("wait times out", async () => {
    const { fetch } = mockFetch({ body: intent({ status: "running" }) });
    await expect(make(fetch).intents.wait("int_1", { intervalMs: 5, timeoutMs: 1 })).rejects.toBeInstanceOf(WaitTimeoutError);
  });

  it("gets the balance", async () => {
    const { fetch, calls } = mockFetch({ body: { object: "balance", available: 24450, currency: "usd" } });
    const b = await make(fetch).balance.get();
    expect(b.available).toBe(24450);
    expect(calls[0]!.url).toBe("https://api.test/v1/balance");
  });
});
