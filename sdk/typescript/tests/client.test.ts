import { describe, expect, it, vi } from "vitest";

import { OpenIntents, OpenIntentsError, WaitTimeoutError, type InputRequest, type Intent } from "../src/index.js";

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
          payment: { reason: "add_money", payment_url: "https://pay/p", total: 550, fee: 0, currency: "usd", expires_at: "x" },
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

  it("wait returns at requires_login by default, with the link to pass on", async () => {
    const { fetch } = mockFetch(
      { body: intent({ status: "running" }) },
      {
        body: intent({
          status: "requires_login",
          sign_in: { url: "https://openintents.io/signin/sgn_1", shop: "amazon.com", expires_at: "x" },
        }),
      },
    );
    const got = await make(fetch).intents.wait("int_1", { intervalMs: 1 });
    expect(got.status).toBe("requires_login");
    expect(got.sign_in?.url).toBe("https://openintents.io/signin/sgn_1");
    expect(got.sign_in?.shop).toBe("amazon.com");
  });

  it("wait keeps polling past a sign-in when asked", async () => {
    const { fetch } = mockFetch(
      { body: intent({ status: "requires_login" }) },
      { body: intent({ status: "queued" }) },
      { body: intent({ status: "completed" }) },
    );
    const got = await make(fetch).intents.wait("int_1", { intervalMs: 1, returnOnSignIn: false });
    expect(got.status).toBe("completed");
  });

  it("sends sign_in_link when creating an intent", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: intent({ status: "queued" }) });
    await make(fetch).intents.create({ intent: "a lamp", sign_in_link: false });
    expect(JSON.parse(String(calls[0]!.init?.body))).toMatchObject({ intent: "a lamp", sign_in_link: false });
  });

  it("wait returns at requires_input by default, with what to ask", async () => {
    const inputRequest = {
      id: "inp_1",
      kind: "code" as const,
      shop: "blinkit.com",
      prompt: "Enter the code blinkit.com just sent to your phone.",
      options: [],
      choices: [],
      fields: [],
      expects: "text" as const,
      channel: "sms" as const,
      length: 6,
      url: "https://openintents.io/input/inp_1",
      expires_at: "x",
      attempts_left: 2,
    };
    const { fetch } = mockFetch(
      { body: intent({ status: "running" }) },
      { body: intent({ status: "requires_input", input_request: inputRequest }) },
    );
    const got = await make(fetch).intents.wait("int_1", { intervalMs: 1 });
    expect(got.status).toBe("requires_input");
    expect(got.input_request).toEqual(inputRequest);
  });

  it("wait returns a detail with its options, answered with one of them", async () => {
    const detail = {
      id: "inp_2",
      kind: "detail" as const,
      shop: "flipkart.com",
      prompt: "Which size would you like?",
      options: ["S", "M", "L"],
      choices: [],
      fields: [],
      expects: "text" as const,
      channel: null,
      length: null,
      url: "https://openintents.io/input/inp_2",
      expires_at: "x",
      attempts_left: 2,
    };
    const { fetch, calls } = mockFetch(
      { body: intent({ status: "requires_input", input_request: detail }) },
      { body: intent({ status: "running" }) },
    );
    const oi = make(fetch);
    const got = await oi.intents.wait("int_1", { intervalMs: 1 });
    expect(got.input_request).toEqual(detail);
    expect(got.input_request?.kind).toBe("detail");
    expect(got.input_request?.options).toEqual(["S", "M", "L"]);
    await oi.intents.input("int_1", { input: got.input_request!.id, value: "M" });
    expect(JSON.parse(calls[1]!.init.body as string)).toEqual({ input: "inp_2", value: "M" });
  });

  it("wait keeps polling past an input request when asked", async () => {
    const { fetch } = mockFetch(
      { body: intent({ status: "requires_input" }) },
      { body: intent({ status: "running" }) },
      { body: intent({ status: "completed" }) },
    );
    const got = await make(fetch).intents.wait("int_1", { intervalMs: 1, returnOnInput: false });
    expect(got.status).toBe("completed");
  });

  it("sends sign_in_agent and sign_in_hint when creating an intent", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: intent({ status: "queued" }) });
    await make(fetch).intents.create({ intent: "a lamp", sign_in_agent: false, sign_in_hint: { phone: "+91 98765 43210" } });
    expect(JSON.parse(String(calls[0]!.init?.body))).toEqual({
      intent: "a lamp",
      sign_in_agent: false,
      sign_in_hint: { phone: "+91 98765 43210" },
    });
  });

  it("sends pay_at_shop and reads a payment request's image_url", async () => {
    const payment: InputRequest = {
      id: "inp_1", kind: "payment", shop: "blinkit.com", prompt: "Pay 249.00 INR", options: [], choices: [], fields: [],
      expects: "text", channel: null, length: null, url: "https://openintents.io/input/inp_1",
      image_url: "https://openintents.io/input/inp_1/code.jpg", expires_at: "2026-10-01T00:05:00Z", attempts_left: 1,
    };
    const { fetch, calls } = mockFetch({ status: 201, body: intent({ status: "requires_input", input_request: payment }) });
    const got = await make(fetch).intents.create({ intent: "a lamp", pay_at_shop: true });
    expect(JSON.parse(String(calls[0]!.init?.body))).toEqual({ intent: "a lamp", pay_at_shop: true });
    expect(got.input_request?.kind).toBe("payment");
    expect(got.input_request?.image_url).toBe("https://openintents.io/input/inp_1/code.jpg");
  });

  it("answers an input request with a value, a decline or done", async () => {
    const running = { body: intent({ status: "running" }) };
    const { fetch, calls } = mockFetch(running, running, running);
    const oi = make(fetch);
    const got = await oi.intents.input("int_1", { input: "inp_1", value: "481516" });
    expect(got.status).toBe("running");
    const { url, init } = calls[0]!;
    expect(url).toBe("https://api.test/v1/intents/int_1/input");
    expect(init.method).toBe("POST");
    expect((init.headers as Record<string, string>)["Idempotency-Key"]).toMatch(/^[0-9a-f-]{36}$/);
    expect(JSON.parse(init.body as string)).toEqual({ input: "inp_1", value: "481516" });
    await oi.intents.input("int_1", { input: "inp_1", decline: true });
    expect(JSON.parse(calls[1]!.init.body as string)).toEqual({ input: "inp_1", decline: true });
    await oi.intents.input("int_1", { input: "inp_2", done: true });
    expect(JSON.parse(calls[2]!.init.body as string)).toEqual({ input: "inp_2", done: true });
  });

  it("throws input_not_waiting for an answer that comes too late", async () => {
    const { fetch } = mockFetch({
      status: 409,
      body: { error: { type: "conflict", code: "input_not_waiting", message: "That request has expired." } },
    });
    const err = await make(fetch)
      .intents.input("int_1", { input: "inp_1", value: "481516" })
      .catch((e: unknown) => e);
    expect(err).toBeInstanceOf(OpenIntentsError);
    expect((err as OpenIntentsError).code).toBe("input_not_waiting");
  });

  it("wait times out", async () => {
    const { fetch } = mockFetch({ body: intent({ status: "running" }) });
    await expect(make(fetch).intents.wait("int_1", { intervalMs: 5, timeoutMs: 1 })).rejects.toBeInstanceOf(WaitTimeoutError);
  });

  it("buys for a customer", async () => {
    const { fetch, calls } = mockFetch(
      { status: 201, body: intent({ customer: "cus_1" }) },
      { body: { object: "list", data: [], next_cursor: null } },
    );
    const oi = make(fetch);
    const created = await oi.intents.create({ intent: "flat white", customer: "cus_1" });
    expect(created.customer).toBe("cus_1");
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({ intent: "flat white", customer: "cus_1" });
    await oi.intents.list({ customer: "cus_1" });
    expect(calls[1]!.url).toBe("https://api.test/v1/intents?customer=cus_1");
  });

  it("creates, updates and lists customers", async () => {
    const customer = { id: "cus_1", object: "customer", external_id: "u-42", name: null, email: null, metadata: {}, created_at: "x" };
    const { fetch, calls } = mockFetch(
      { status: 201, body: customer },
      { body: { ...customer, name: "Ada" } },
      { body: { object: "list", data: [customer], next_cursor: null } },
      { body: customer },
    );
    const oi = make(fetch);
    await oi.customers.create({ external_id: "u-42" });
    expect(calls[0]!.url).toBe("https://api.test/v1/customers");
    expect(calls[0]!.init.method).toBe("POST");
    expect((calls[0]!.init.headers as Record<string, string>)["Idempotency-Key"]).toMatch(/^[0-9a-f-]{36}$/);
    const updated = await oi.customers.update("cus_1", { name: "Ada" });
    expect(updated.name).toBe("Ada");
    expect(calls[1]!.init.method).toBe("PATCH");
    expect(calls[1]!.url).toBe("https://api.test/v1/customers/cus_1");
    expect(JSON.parse(calls[1]!.init.body as string)).toEqual({ name: "Ada" });
    const page = await oi.customers.list({ external_id: "u-42" });
    expect(page.data[0]!.external_id).toBe("u-42");
    expect(calls[2]!.url).toBe("https://api.test/v1/customers?external_id=u-42");
    await oi.customers.get("cus_1");
    expect(calls[3]!.url).toBe("https://api.test/v1/customers/cus_1");
  });
});
