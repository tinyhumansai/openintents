import { describe, expect, it, vi } from "vitest";

import { OpenIntents, OpenIntentsError, type Profile, type Session } from "../src/index.js";

interface Reply {
  status?: number;
  body?: unknown;
  raw?: BodyInit | null;
  headers?: Record<string, string>;
}

/** Answers each call with the next reply; the last one repeats. */
function mockFetch(...replies: Reply[]) {
  const calls: Array<{ url: string; init: RequestInit }> = [];
  const fn = vi.fn(async (url: URL | RequestInfo, init?: RequestInit) => {
    calls.push({ url: String(url), init: init ?? {} });
    const r = replies.length > 1 ? replies.shift()! : replies[0]!;
    const body = r.raw !== undefined ? r.raw : JSON.stringify(r.body);
    return new Response(body, { status: r.status ?? 200, headers: r.headers });
  });
  return { fetch: fn as unknown as typeof fetch, calls };
}

const make = (f: typeof fetch) =>
  new OpenIntents({ apiKey: "oi_test_x", baseUrl: "https://api.test/v1", fetch: f, maxRetries: 0 });

const header = (call: { init: RequestInit } | undefined, name: string) =>
  (call?.init.headers as Record<string, string>)[name];

const profile = (over: Partial<Profile> = {}): Profile => ({
  id: "prf_1",
  object: "profile",
  customer: "cus_1",
  status: "ready",
  lease: null,
  size_bytes: null,
  created_at: "2026-10-01T08:30:00Z",
  last_used_at: null,
  ...over,
});

const session = (over: Partial<Session> = {}): Session => ({
  id: "ses_1",
  object: "session",
  profile: "prf_1",
  customer: "cus_1",
  status: "queued",
  test: true,
  created_at: "2026-10-01T08:30:00Z",
  expires_at: "2026-10-01T08:45:00Z",
  ...over,
});

const list = (data: unknown[], next_cursor: string | null = null) => ({ body: { object: "list", data, next_cursor } });

describe("profiles", () => {
  it("lists, makes, retrieves and forgets profiles", async () => {
    const { fetch, calls } = mockFetch(
      list([profile()]),
      { status: 201, body: profile() },
      { status: 200, body: profile({ customer: null }) },
      { body: profile() },
      { status: 202, body: profile({ status: "forgetting" }) },
    );
    const oi = make(fetch);

    const page = await oi.profiles.list({ customer: "cus_1", limit: 5 });
    expect(page.data[0]!.id).toBe("prf_1");
    expect(calls[0]!.url).toBe("https://api.test/v1/profiles?customer=cus_1&limit=5");

    await oi.profiles.create({ customer: "cus_1" });
    expect(calls[1]!.init.method).toBe("POST");
    expect(calls[1]!.url).toBe("https://api.test/v1/profiles");
    expect(JSON.parse(calls[1]!.init.body as string)).toEqual({ customer: "cus_1" });
    // Making a profile is idempotent by itself: no key.
    expect(header(calls[1], "Idempotency-Key")).toBeUndefined();

    const own = await oi.profiles.create();
    expect(own.customer).toBeNull();
    expect(JSON.parse(calls[2]!.init.body as string)).toEqual({});

    await oi.profiles.get("prf_1");
    expect(calls[3]!.url).toBe("https://api.test/v1/profiles/prf_1");

    const forgetting = await oi.profiles.forget("prf_1");
    expect(forgetting.status).toBe("forgetting");
    expect(calls[4]!.init.method).toBe("DELETE");
    expect(calls[4]!.url).toBe("https://api.test/v1/profiles/prf_1");
  });

  it("follows cursors in listAll", async () => {
    const { fetch, calls } = mockFetch(list([profile({ id: "a" })], "a"), list([profile({ id: "b" })]));
    const ids: string[] = [];
    for await (const p of make(fetch).profiles.listAll({ customer: "cus_1" })) ids.push(p.id);
    expect(ids).toEqual(["a", "b"]);
    expect(calls[1]!.url).toBe("https://api.test/v1/profiles?customer=cus_1&cursor=a");
  });
});

describe("sessions", () => {
  it("opens, lists, retrieves and releases sessions", async () => {
    const { fetch, calls } = mockFetch(
      { status: 201, body: session() },
      list([session({ status: "open" })], "ses_1"),
      list([session({ id: "ses_0", status: "open" })]),
      { body: session({ status: "open", url: "https://shop.example/login" }) },
      { body: session({ status: "closing" }) },
    );
    const oi = make(fetch);

    const opened = await oi.sessions.create({ customer: "cus_1", start_url: "https://shop.example/login", max_minutes: 10 });
    expect(opened.status).toBe("queued");
    expect(calls[0]!.url).toBe("https://api.test/v1/sessions");
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({
      customer: "cus_1",
      start_url: "https://shop.example/login",
      max_minutes: 10,
    });
    expect(header(calls[0], "Idempotency-Key")).toMatch(/^[0-9a-f-]{36}$/);

    const ids: string[] = [];
    for await (const s of oi.sessions.listAll({ status: "open", profile: "prf_1", customer: "cus_1", limit: 1 })) {
      ids.push(s.id);
    }
    expect(ids).toEqual(["ses_1", "ses_0"]);
    expect(calls[1]!.url).toBe("https://api.test/v1/sessions?status=open&profile=prf_1&customer=cus_1&limit=1");
    expect(calls[2]!.url).toContain("cursor=ses_1");

    const got = await oi.sessions.get("ses_1");
    expect(got.url).toBe("https://shop.example/login");
    expect(calls[3]!.url).toBe("https://api.test/v1/sessions/ses_1");

    const released = await oi.sessions.release("ses_1");
    expect(released.status).toBe("closing");
    expect(calls[4]!.init.method).toBe("POST");
    expect(calls[4]!.url).toBe("https://api.test/v1/sessions/ses_1/release");
    expect(header(calls[4], "Idempotency-Key")).toBeTruthy();
  });

  it("runs commands, answered done or still pending", async () => {
    const result = { id: "cmd_1", object: "session_command", created_at: "x" };
    const { fetch, calls } = mockFetch(
      { body: { ...result, type: "act", status: "done", url: "https://shop.example/login" } },
      { status: 202, body: { ...result, type: "navigate", status: "pending" } },
      { body: { ...result, type: "navigate", status: "done", completed_at: "y" } },
      {
        body: {
          ...result,
          type: "act",
          status: "refused",
          error: { code: "payment_not_allowed", message: "A session never pays." },
        },
      },
    );
    const oi = make(fetch);

    const typed = await oi.sessions.runCommand("ses_1", { type: "act", ref: "e12", action: "type", text: "ada@example.com" });
    expect(typed.status).toBe("done");
    expect(calls[0]!.url).toBe("https://api.test/v1/sessions/ses_1/commands");
    expect(JSON.parse(calls[0]!.init.body as string)).toEqual({
      type: "act",
      ref: "e12",
      action: "type",
      text: "ada@example.com",
    });
    expect(header(calls[0], "Idempotency-Key")).toMatch(/^[0-9a-f-]{36}$/);

    // A 202 still returns the command, `pending`.
    const pending = await oi.sessions.runCommand(
      "ses_1",
      { type: "navigate", url: "https://shop.example/cart" },
      { idempotencyKey: "nav-1" },
    );
    expect(pending.status).toBe("pending");
    expect(header(calls[1], "Idempotency-Key")).toBe("nav-1");

    const done = await oi.sessions.getCommand("ses_1", "cmd_1");
    expect(done.status).toBe("done");
    expect(calls[2]!.url).toBe("https://api.test/v1/sessions/ses_1/commands/cmd_1");

    const refused = await oi.sessions.runCommand("ses_1", { type: "act", ref: "e40", action: "click" });
    expect(refused.status).toBe("refused");
    expect(refused.error?.code).toBe("payment_not_allowed");
  });

  it("returns a screenshot as JPEG bytes", async () => {
    const jpeg = new Uint8Array([0xff, 0xd8, 0xff, 0xe0]);
    const { fetch, calls } = mockFetch({ raw: jpeg, headers: { "content-type": "image/jpeg" } });
    const shot = await make(fetch).sessions.screenshot("ses_1");
    expect(shot).toBeInstanceOf(Uint8Array);
    expect([...shot]).toEqual([0xff, 0xd8, 0xff, 0xe0]);
    expect(calls[0]!.url).toBe("https://api.test/v1/sessions/ses_1/screenshot");
    expect(header(calls[0], "Accept")).toBe("image/jpeg");
  });

  it("surfaces the lease of a busy profile and the code of other errors", async () => {
    const lease = { holder: { type: "intent", id: "int_1" }, expires_at: "2026-10-01T08:31:00Z" };
    const { fetch } = mockFetch(
      { status: 409, body: { error: { type: "conflict", code: "profile_busy", message: "Profile in use.", lease } } },
      { status: 409, body: { error: { type: "conflict", code: "session_not_open", message: "Not open." } } },
    );
    const oi = make(fetch);

    const busy = await oi.sessions.create().catch((e: unknown) => e);
    expect(busy).toBeInstanceOf(OpenIntentsError);
    expect(busy).toMatchObject({ status: 409, type: "conflict", code: "profile_busy", lease });

    const closed = await oi.sessions.screenshot("ses_1").catch((e: unknown) => e);
    expect(closed).toMatchObject({ status: 409, code: "session_not_open", lease: undefined });
  });
});
