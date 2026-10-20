import { describe, expect, it, vi } from "vitest";

import { OpenIntents, OpenIntentsError } from "../src/index.js";

function mockFetch(...responses: Array<{ status?: number; body?: unknown; headers?: Record<string, string> }>) {
  const calls: Array<{ url: string; init: RequestInit }> = [];
  const fn = vi.fn(async (url: URL | RequestInfo, init?: RequestInit) => {
    calls.push({ url: String(url), init: init ?? {} });
    const r = responses.shift() ?? responses.at(-1)!;
    const body = r.body === undefined ? null : JSON.stringify(r.body);
    return new Response(r.status === 204 ? null : body, { status: r.status ?? 200, headers: r.headers });
  });
  return { fetch: fn as unknown as typeof fetch, calls };
}

const make = (f: typeof fetch, extra = {}) =>
  new OpenIntents({ apiKey: "oi_test_x", baseUrl: "https://api.test/v1", fetch: f, maxRetries: 0, ...extra });

const hdr = (call: { init: RequestInit }, name: string) => (call.init.headers as Record<string, string>)[name];

describe("holding a request with Prefer: wait", () => {
  it("get sends Prefer: wait=N", async () => {
    const { fetch, calls } = mockFetch({ body: { id: "int_1", status: "completed" } });
    await make(fetch).intents.get("int_1", { wait: 30 });
    expect(hdr(calls[0]!, "Prefer")).toBe("wait=30");
  });

  it("create sends Prefer: wait=N and still an idempotency key", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: { id: "int_1", status: "queued" } });
    await make(fetch).intents.create({ intent: "flat white", country: "gb", end_user_ip: "203.0.113.9" }, { wait: 20 });
    expect(hdr(calls[0]!, "Prefer")).toBe("wait=20");
    expect(hdr(calls[0]!, "Idempotency-Key")).toBeTruthy();
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({
      intent: "flat white",
      country: "gb",
      end_user_ip: "203.0.113.9",
    });
  });

  it("sends no Prefer header without wait", async () => {
    const { fetch, calls } = mockFetch({ body: { id: "int_1" } });
    await make(fetch).intents.get("int_1");
    expect(hdr(calls[0]!, "Prefer")).toBeUndefined();
  });

  it("lets a held request outlast the client timeout", async () => {
    let seen: AbortSignal | undefined;
    const slow = (async (_url: unknown, init?: RequestInit) => {
      seen = init?.signal ?? undefined;
      await new Promise((r) => setTimeout(r, 60));
      if (seen?.aborted) throw seen.reason;
      return new Response(JSON.stringify({ id: "int_1", status: "completed" }));
    }) as typeof fetch;
    // A 20 ms timeout would cut a 60 ms answer off, but the server was asked to hold for 1 s.
    const got = await make(slow, { timeoutMs: 20 }).intents.get("int_1", { wait: 1 });
    expect(got.status).toBe("completed");
  });
});

describe("answering with a form", () => {
  it("posts values by field name", async () => {
    const { fetch, calls } = mockFetch({ body: { id: "int_1", status: "running" } });
    await make(fetch).intents.input("int_1", { input: "inp_1", values: { seat_row: "12", seat: "a" } });
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({
      input: "inp_1",
      values: { seat_row: "12", seat: "a" },
    });
  });
});

describe("errors", () => {
  it("carry Retry-After and the rate limit headers of a 429", async () => {
    const { fetch } = mockFetch({
      status: 429,
      body: { error: { type: "rate_limit", code: "rate_limit_exceeded", message: "slow down" } },
      headers: { "Retry-After": "7", "RateLimit-Limit": "300", "RateLimit-Remaining": "0", "RateLimit-Reset": "7" },
    });
    const err = await make(fetch).intents.get("int_1").catch((e) => e);
    expect(err).toBeInstanceOf(OpenIntentsError);
    expect(err.code).toBe("rate_limit_exceeded");
    expect(err.retryAfter).toBe(7);
    expect(err.rateLimit).toEqual({ limit: 300, remaining: 0, reset: 7 });
  });

  it("leave retryAfter and rateLimit out when the headers are absent", async () => {
    const { fetch } = mockFetch({ status: 404, body: { error: { type: "not_found", message: "no" } } });
    const err = await make(fetch).intents.get("x").catch((e) => e);
    expect(err.retryAfter).toBeUndefined();
    expect(err.rateLimit).toBeUndefined();
  });
});

describe("customers", () => {
  it("makes a portal session with no idempotency key", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: { object: "portal_session", url: "https://x/p" } });
    const got = await make(fetch).customers.createPortalSession("cus_1", { ttl_seconds: 3600, country: "de" });
    expect(got.url).toBe("https://x/p");
    expect(calls[0]!.url).toBe("https://api.test/v1/customers/cus_1/portal_sessions");
    expect(calls[0]!.init.method).toBe("POST");
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({ ttl_seconds: 3600, country: "de" });
    expect(hdr(calls[0]!, "Idempotency-Key")).toBeUndefined();
  });

  it("sends {} when a portal session takes the defaults", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: {} });
    await make(fetch).customers.createPortalSession("cus_1");
    expect(calls[0]!.init.body).toBe("{}");
  });

  it("asks a customer for their secrets, and reads where the request stands", async () => {
    const { fetch, calls } = mockFetch(
      { status: 201, body: { object: "secret_request", id: "sr_1", url: "https://x/s" } },
      { body: { object: "secret_request", id: "sr_1", state: "done", secrets: ["sec_1"] } },
    );
    const oi = make(fetch);
    const made = await oi.customers.createSecretRequest("cus_1", { kinds: ["address", "contact"] });
    expect(made.url).toBe("https://x/s");
    expect(calls[0]!.url).toBe("https://api.test/v1/customers/cus_1/secret_requests");
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({ kinds: ["address", "contact"] });
    const got = await oi.secretRequests.get("sr_1");
    expect(got.secrets).toEqual(["sec_1"]);
    expect(calls[1]!.url).toBe("https://api.test/v1/secret_requests/sr_1");
  });

  it("sets spending limits on create", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: { id: "cus_1" } });
    await make(fetch).customers.create({ limits: { per_intent: 5000, monthly: null } });
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({ limits: { per_intent: 5000, monthly: null } });
  });
});

describe("secrets", () => {
  it("stores one with an idempotency key", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: { id: "sec_1", object: "secret", mask: "Paris, FR" } });
    const got = await make(fetch).secrets.create({
      kind: "address",
      label: "Home",
      fields: { recipient_name: "A", line1: "1 St", postal_code: "N1", city: "London", country: "gb" },
      shops: ["amazon.com"],
    });
    expect(got.id).toBe("sec_1");
    expect(calls[0]!.url).toBe("https://api.test/v1/secrets");
    expect(calls[0]!.init.method).toBe("POST");
    expect(hdr(calls[0]!, "Idempotency-Key")).toBeTruthy();
  });

  it("lists by customer and kind, reads one, replaces one and reads its history", async () => {
    const list = { object: "list", data: [], next_cursor: null };
    const { fetch, calls } = mockFetch(
      { body: list },
      { body: { id: "sec_1" } },
      { body: { id: "sec_1", version: 2 } },
      { body: { object: "list", data: [], next_cursor: null } },
    );
    const oi = make(fetch);
    await oi.secrets.list({ customer: "cus_1", kind: "loyalty" });
    expect(calls[0]!.url).toBe("https://api.test/v1/secrets?customer=cus_1&kind=loyalty");
    await oi.secrets.get("sec_1");
    expect(calls[1]!.url).toBe("https://api.test/v1/secrets/sec_1");
    await oi.secrets.replace("sec_1", { kind: "contact", label: "Me", fields: { email: "a@b.co" } });
    expect(calls[2]!.url).toBe("https://api.test/v1/secrets/sec_1");
    expect(calls[2]!.init.method).toBe("PUT");
    expect(hdr(calls[2]!, "Idempotency-Key")).toBeTruthy();
    await oi.secrets.uses("sec_1", { limit: 5 });
    expect(calls[3]!.url).toBe("https://api.test/v1/secrets/sec_1/uses?limit=5");
  });

  it("deletes one and resolves with nothing", async () => {
    const { fetch, calls } = mockFetch({ status: 204 });
    await expect(make(fetch).secrets.delete("sec_1")).resolves.toBeUndefined();
    expect(calls[0]!.init.method).toBe("DELETE");
  });
});

describe("sign-in links and live links", () => {
  it("makes a sign-in link and reads it", async () => {
    const { fetch, calls } = mockFetch(
      { status: 201, body: { id: "sgn_1", object: "sign_in", url: "https://x/signin/sgn_1" } },
      { body: { id: "sgn_1", status: "done" } },
    );
    const oi = make(fetch);
    const made = await oi.signIns.create({ shop_url: "https://www.amazon.com/", customer: "cus_1" });
    expect(made.url).toBe("https://x/signin/sgn_1");
    expect(calls[0]!.url).toBe("https://api.test/v1/sign_ins");
    expect(hdr(calls[0]!, "Idempotency-Key")).toBeTruthy();
    expect((await oi.signIns.get("sgn_1")).status).toBe("done");
    expect(calls[1]!.url).toBe("https://api.test/v1/sign_ins/sgn_1");
  });

  it("makes a live link for a session with no body", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: { object: "live_link", url: "https://x/live" } });
    const got = await make(fetch).sessions.createLiveLink("ses_1");
    expect(got.url).toBe("https://x/live");
    expect(calls[0]!.url).toBe("https://api.test/v1/sessions/ses_1/live_links");
    expect(calls[0]!.init.method).toBe("POST");
    expect(calls[0]!.init.body).toBeUndefined();
  });
});

describe("webhook endpoints", () => {
  it("registers one and keeps the secret from the response", async () => {
    const { fetch, calls } = mockFetch({ status: 201, body: { id: "we_1", secret: "whsec_a" } });
    const got = await make(fetch).webhookEndpoints.create({ url: "https://h.example/hook", events: ["sign_in.completed"] });
    expect(got.secret).toBe("whsec_a");
    expect(calls[0]!.url).toBe("https://api.test/v1/webhook_endpoints");
    expect(hdr(calls[0]!, "Idempotency-Key")).toBeTruthy();
  });

  it("lists, reads, updates and deletes", async () => {
    const { fetch, calls } = mockFetch(
      { body: { object: "list", data: [], next_cursor: null } },
      { body: { id: "we_1" } },
      { body: { id: "we_1", disabled: true } },
      { status: 204 },
    );
    const oi = make(fetch);
    await oi.webhookEndpoints.list({ limit: 2 });
    expect(calls[0]!.url).toBe("https://api.test/v1/webhook_endpoints?limit=2");
    await oi.webhookEndpoints.get("we_1");
    await oi.webhookEndpoints.update("we_1", { disabled: true });
    expect(calls[2]!.init.method).toBe("PATCH");
    expect(JSON.parse(calls[2]!.init.body as string)).toEqual({ disabled: true });
    await oi.webhookEndpoints.delete("we_1");
    expect(calls[3]!.init.method).toBe("DELETE");
    expect(calls[3]!.url).toBe("https://api.test/v1/webhook_endpoints/we_1");
  });

  it("follows cursors in listAll", async () => {
    const { fetch } = mockFetch(
      { body: { object: "list", data: [{ id: "a" }], next_cursor: "a" } },
      { body: { object: "list", data: [{ id: "b" }], next_cursor: null } },
    );
    const ids: string[] = [];
    for await (const e of make(fetch).webhookEndpoints.listAll()) ids.push(e.id);
    expect(ids).toEqual(["a", "b"]);
  });

  it("rotates the secret, sends a test event, lists deliveries and redelivers one", async () => {
    const { fetch, calls } = mockFetch(
      { body: { id: "we_1", secret: "whsec_b", previous_secret_expires_at: "x" } },
      { status: 201, body: { id: "whd_1", status: "pending" } },
      { body: { object: "list", data: [], next_cursor: null } },
      { status: 201, body: { id: "whd_2", redelivery_of: "whd_1" } },
    );
    const oi = make(fetch);
    expect((await oi.webhookEndpoints.rotateSecret("we_1")).secret).toBe("whsec_b");
    expect(calls[0]!.url).toBe("https://api.test/v1/webhook_endpoints/we_1/rotate_secret");
    expect(calls[0]!.init.method).toBe("POST");
    expect((await oi.webhookEndpoints.test("we_1")).status).toBe("pending");
    expect(calls[1]!.url).toBe("https://api.test/v1/webhook_endpoints/we_1/test");
    await oi.webhookEndpoints.listDeliveries("we_1", { limit: 10 });
    expect(calls[2]!.url).toBe("https://api.test/v1/webhook_endpoints/we_1/deliveries?limit=10");
    const again = await oi.webhookEndpoints.redeliver("we_1", "whd_1");
    expect(again.redelivery_of).toBe("whd_1");
    expect(calls[3]!.url).toBe("https://api.test/v1/webhook_endpoints/we_1/deliveries/whd_1/redeliver");
  });
});
