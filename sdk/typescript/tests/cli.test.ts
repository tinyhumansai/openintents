import { readFileSync } from "node:fs";
import { mkdtemp, readFile, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it, vi } from "vitest";

import { describeIntent } from "../src/cli-format.js";
import { runCli, type CliIo } from "../src/cli-main.js";
import { OpenIntents } from "../src/index.js";
import type { Intent } from "../src/types.js";

type Reply = { status?: number; body?: unknown; bytes?: Uint8Array };

/** Runs the CLI against a scripted API and collects what it prints. */
async function cli(argv: string[], replies: Reply[] = [{ body: {} }], over: Partial<CliIo> = {}) {
  const calls: Array<{ method: string; url: string; body: unknown }> = [];
  const queue = [...replies];
  const fetchImpl = vi.fn(async (url: URL | RequestInfo, init?: RequestInit) => {
    calls.push({
      method: init?.method ?? "GET",
      url: String(url),
      body: init?.body ? JSON.parse(init.body as string) : undefined,
    });
    const r = queue.shift() ?? replies.at(-1)!;
    if (r.bytes) return new Response(r.bytes as unknown as BodyInit, { status: 200, headers: { "Content-Type": "image/jpeg" } });
    return new Response(r.status === 204 ? null : JSON.stringify(r.body ?? {}), { status: r.status ?? 200 });
  });
  const out: string[] = [];
  const err: string[] = [];
  const code = await runCli(argv, {
    client: async () =>
      new OpenIntents({ apiKey: "oi_test_x", baseUrl: "https://api.test/v1", fetch: fetchImpl as unknown as typeof fetch, maxRetries: 0 }),
    out: (s) => out.push(s),
    err: (s) => err.push(s),
    stdin: async () => "",
    openUrl: () => {},
    configFile: join(tmpdir(), "openintents-cli-test-unused.json"),
    ...over,
  });
  return { code, out: out.join("\n"), err: err.join("\n"), calls };
}

const customer = { id: "cus_1", object: "customer", external_id: "u-7", name: "Ada", email: "ada@example.com", created_at: "2026-10-01T00:00:00Z" };
const session = { id: "ses_1", object: "session", profile: "prf_1", status: "open", test: true, created_at: "2026-10-01T00:00:00Z", expires_at: "2026-10-01T00:15:00Z", url: "https://shop.example/" };
const profile = { id: "prf_1", object: "profile", customer: "cus_1", status: "idle", created_at: "2026-10-01T00:00:00Z" };
const secret = { id: "sec_1", object: "secret", customer: null, kind: "loyalty", label: "Flying Blue", mask: "••7890", public: { program: "Flying Blue" }, shops: ["airfrance.com"], default: false, mode: "test", entered_by: "developer", version: 1, created_at: "2026-10-01T00:00:00Z", updated_at: "2026-10-01T00:00:00Z", expires_at: null, last_used_at: null };

describe("describeIntent", () => {
  const asking = (kind: string) =>
    describeIntent({
      id: "int_1",
      status: "requires_input",
      intent: "a black t-shirt",
      input_request: { id: "inp_1", kind, shop: "amazon.in", prompt: "p", url: "https://x/input/inp_1" },
    } as unknown as Intent);

  it("suggests --done for a question that takes no value, a check among them", () => {
    for (const kind of ["approval", "payment", "check"]) {
      expect(asking(kind)).toContain("--input inp_1 --done (or --decline)");
    }
    expect(asking("code")).toContain("--input inp_1 --value <answer>");
  });
});

describe("openintents customers", () => {
  it("creates a customer from flags, limits in dollars", async () => {
    const r = await cli(
      ["customers", "create", "--external-id", "u-7", "--name", "Ada", "--email", "ada@example.com", "--metadata", "plan=pro", "--per-intent", "50", "--monthly", "none"],
      [{ status: 201, body: customer }],
    );
    expect(r.code).toBe(0);
    expect(r.calls[0]).toMatchObject({
      method: "POST",
      url: "https://api.test/v1/customers",
      body: { external_id: "u-7", name: "Ada", email: "ada@example.com", metadata: { plan: "pro" }, limits: { per_intent: 5000, monthly: null } },
    });
    expect(r.out).toContain("cus_1");
    expect(r.out).toContain("u-7");
  });

  it("gets, updates and lists customers", async () => {
    const got = await cli(["customers", "get", "cus_1"], [{ body: customer }]);
    expect(got.calls[0]).toMatchObject({ method: "GET", url: "https://api.test/v1/customers/cus_1" });
    const upd = await cli(["customers", "update", "cus_1", "--name", "Ada L"], [{ body: customer }]);
    expect(upd.calls[0]).toMatchObject({ method: "PATCH", url: "https://api.test/v1/customers/cus_1", body: { name: "Ada L" } });
    const list = await cli(["customers", "list", "--external-id", "u-7", "--limit", "5"], [{ body: { object: "list", data: [customer] } }]);
    expect(list.calls[0]!.url).toBe("https://api.test/v1/customers?external_id=u-7&limit=5");
    expect(list.out).toContain("cus_1");
  });

  it("makes a portal link for the customer only", async () => {
    const r = await cli(
      ["customers", "portal", "cus_1", "--return-url", "https://app.example/back", "--country", "de", "--ttl", "3600"],
      [{ status: 201, body: { id: "por_1", object: "portal_session", customer: "cus_1", country: "de", country_source: "explicit", url: "https://openintents.io/portal/abc", expires_at: "2026-10-01T00:30:00Z" } }],
    );
    expect(r.calls[0]).toMatchObject({
      method: "POST",
      url: "https://api.test/v1/customers/cus_1/portal_sessions",
      body: { return_url: "https://app.example/back", country: "de", ttl_seconds: 3600 },
    });
    expect(r.out).toContain("https://openintents.io/portal/abc");
  });

  it("refuses a customer subcommand it does not know", async () => {
    const r = await cli(["customers", "explode"]);
    expect(r.code).toBe(1);
    expect(r.err).toMatch(/customers/);
    expect(r.calls).toHaveLength(0);
  });
});

describe("openintents sessions", () => {
  it("opens a session on a customer's profile", async () => {
    const r = await cli(
      ["sessions", "open", "--customer", "cus_1", "--start-url", "https://shop.example/", "--max-minutes", "5", "--country", "jp"],
      [{ status: 201, body: session }],
    );
    expect(r.code).toBe(0);
    expect(r.calls[0]).toMatchObject({
      method: "POST",
      url: "https://api.test/v1/sessions",
      body: { customer: "cus_1", start_url: "https://shop.example/", max_minutes: 5, country: "jp" },
    });
    expect(r.out).toContain("ses_1");
  });

  it("lists, gets and releases sessions", async () => {
    const list = await cli(["sessions", "list", "--status", "open", "--customer", "cus_1"], [{ body: { object: "list", data: [session] } }]);
    expect(list.calls[0]!.url).toBe("https://api.test/v1/sessions?status=open&customer=cus_1");
    const got = await cli(["sessions", "get", "ses_1"], [{ body: session }]);
    expect(got.calls[0]).toMatchObject({ method: "GET", url: "https://api.test/v1/sessions/ses_1" });
    const rel = await cli(["sessions", "release", "ses_1"], [{ body: { ...session, status: "closed" } }]);
    expect(rel.calls[0]).toMatchObject({ method: "POST", url: "https://api.test/v1/sessions/ses_1/release" });
    expect(rel.out).toContain("closed");
  });

  it("makes a live link", async () => {
    const r = await cli(
      ["sessions", "live", "ses_1"],
      [{ status: 201, body: { object: "live_link", session: "ses_1", url: "https://openintents.io/live/xyz", expires_at: "2026-10-01T00:10:00Z" } }],
    );
    expect(r.calls[0]).toMatchObject({ method: "POST", url: "https://api.test/v1/sessions/ses_1/live_links" });
    expect(r.out).toContain("https://openintents.io/live/xyz");
  });

  it.each([
    [["navigate", "https://shop.example/cart"], { type: "navigate", url: "https://shop.example/cart" }],
    [["snapshot"], { type: "snapshot" }],
    [["read"], { type: "read" }],
    [["read", "main"], { type: "read", selector: "main" }],
    [["click", "e12"], { type: "act", ref: "e12", action: "click" }],
    [["type", "e3", "hello world"], { type: "act", ref: "e3", action: "type", text: "hello world" }],
    [["select", "e4", "M"], { type: "act", ref: "e4", action: "select", value: "M" }],
    [["press", "e4", "Enter"], { type: "act", ref: "e4", action: "press", key: "Enter" }],
  ])("runs the session command %j", async (words, body) => {
    const r = await cli(
      ["sessions", "command", "ses_1", ...words],
      [{ body: { id: "cmd_1", object: "session_command", type: "x", status: "done", url: "https://shop.example/", snapshot: "- button e12", text: "page text", created_at: "2026-10-01T00:00:00Z" } }],
    );
    expect(r.code).toBe(0);
    expect(r.calls[0]).toMatchObject({ method: "POST", url: "https://api.test/v1/sessions/ses_1/commands", body });
  });

  it("prints a snapshot's tree and a read's text", async () => {
    const snap = await cli(["sessions", "command", "ses_1", "snapshot"], [{ body: { id: "c", object: "session_command", type: "snapshot", status: "done", snapshot: "- button \"Buy\" [ref=e1]", created_at: "x" } }]);
    expect(snap.out).toContain('button "Buy" [ref=e1]');
    const read = await cli(["sessions", "command", "ses_1", "read"], [{ body: { id: "c", object: "session_command", type: "read", status: "done", text: "Total 12.00", created_at: "x" } }]);
    expect(read.out).toContain("Total 12.00");
  });

  it("exits 1 and says why when a command is refused", async () => {
    const r = await cli(
      ["sessions", "command", "ses_1", "click", "e9"],
      [{ body: { id: "c", object: "session_command", type: "act", status: "refused", error: { code: "payment_not_allowed", message: "sessions never pay" }, created_at: "x" } }],
    );
    expect(r.code).toBe(1);
    expect(r.out).toContain("payment_not_allowed");
  });

  it("needs the arguments a command takes", async () => {
    expect((await cli(["sessions", "command", "ses_1", "click"])).code).toBe(1);
    expect((await cli(["sessions", "command", "ses_1", "dance"])).err).toMatch(/navigate/);
  });

  it("saves a screenshot to a file", async () => {
    const dir = await mkdtemp(join(tmpdir(), "oi-shot-"));
    const file = join(dir, "page.jpg");
    const r = await cli(["sessions", "screenshot", "ses_1", "--out", file], [{ bytes: new Uint8Array([0xff, 0xd8, 0xff, 0xd9]) }]);
    expect(r.code).toBe(0);
    expect(r.calls[0]).toMatchObject({ method: "GET", url: "https://api.test/v1/sessions/ses_1/screenshot" });
    expect([...(await readFile(file))]).toEqual([0xff, 0xd8, 0xff, 0xd9]);
    expect((await cli(["sessions", "screenshot", "ses_1"])).code).toBe(1);
  });
});

describe("openintents profiles", () => {
  it("lists, creates, gets and forgets profiles", async () => {
    const list = await cli(["profiles", "list", "--customer", "cus_1"], [{ body: { object: "list", data: [profile] } }]);
    expect(list.calls[0]!.url).toBe("https://api.test/v1/profiles?customer=cus_1");
    expect(list.out).toContain("prf_1");
    const created = await cli(["profiles", "create", "--customer", "cus_1"], [{ status: 201, body: profile }]);
    expect(created.calls[0]).toMatchObject({ method: "POST", url: "https://api.test/v1/profiles", body: { customer: "cus_1" } });
    const got = await cli(["profiles", "get", "prf_1"], [{ body: profile }]);
    expect(got.calls[0]).toMatchObject({ method: "GET", url: "https://api.test/v1/profiles/prf_1" });
    const forgot = await cli(["profiles", "forget", "prf_1"], [{ body: { ...profile, status: "forgotten" } }]);
    expect(forgot.calls[0]).toMatchObject({ method: "DELETE", url: "https://api.test/v1/profiles/prf_1" });
    expect(forgot.out).toContain("forgotten");
  });
});

describe("openintents sign-ins", () => {
  it("makes a sign-in link and reads it back", async () => {
    const signIn = { id: "sgn_1", object: "sign_in", status: "open", customer: "cus_1", shop: "amazon.com", country: "us", country_source: "default", return_url: null, url: "https://openintents.io/signin/abc", test: true, created_at: "x", expires_at: "y", ended_at: null };
    const r = await cli(
      ["sign-ins", "create", "https://www.amazon.com/", "--customer", "cus_1", "--return-url", "https://app.example/"],
      [{ status: 201, body: signIn }],
    );
    expect(r.calls[0]).toMatchObject({
      method: "POST",
      url: "https://api.test/v1/sign_ins",
      body: { shop_url: "https://www.amazon.com/", customer: "cus_1", return_url: "https://app.example/" },
    });
    expect(r.out).toContain("https://openintents.io/signin/abc");
    const got = await cli(["sign-ins", "get", "sgn_1"], [{ body: { ...signIn, url: undefined, status: "done" } }]);
    expect(got.calls[0]).toMatchObject({ method: "GET", url: "https://api.test/v1/sign_ins/sgn_1" });
    expect(got.out).toContain("done");
  });
});

describe("openintents secrets", () => {
  it("stores a secret without printing or echoing its values", async () => {
    const r = await cli(
      ["secrets", "set", "--kind", "loyalty", "--label", "Flying Blue", "--field", "member_number=1234567890", "--field", "pin=4321", "--field", "program=Flying Blue", "--shop", "airfrance.com", "--default"],
      [{ status: 201, body: secret }],
    );
    expect(r.code).toBe(0);
    expect(r.calls[0]).toMatchObject({
      method: "POST",
      url: "https://api.test/v1/secrets",
      body: { kind: "loyalty", label: "Flying Blue", fields: { member_number: "1234567890", pin: "4321", program: "Flying Blue" }, shops: ["airfrance.com"], default: true },
    });
    expect(r.out).toContain("••7890");
    for (const text of [r.out, r.err]) {
      expect(text).not.toContain("1234567890");
      expect(text).not.toContain("4321");
    }
  });

  it("takes the fields as JSON on stdin so they stay out of the process list", async () => {
    const r = await cli(["secrets", "set", "--kind", "loyalty", "--label", "Flying Blue", "--stdin"], [{ status: 201, body: secret }], {
      stdin: async () => JSON.stringify({ member_number: "1234567890" }),
    });
    expect(r.calls[0]!.body).toMatchObject({ fields: { member_number: "1234567890" } });
    expect(r.out + r.err).not.toContain("1234567890");
  });

  it("does not repeat a field's value in an error", async () => {
    const r = await cli(["secrets", "set", "--kind", "loyalty", "--label", "x", "--field", "1234567890"]);
    expect(r.code).toBe(1);
    expect(r.err).not.toContain("1234567890");
    expect(r.calls).toHaveLength(0);
  });

  it("lists, shows and deletes secrets, never their values", async () => {
    const list = await cli(["secrets", "list", "--kind", "loyalty", "--customer", "cus_1"], [{ body: { object: "list", data: [secret] } }]);
    expect(list.calls[0]!.url).toBe("https://api.test/v1/secrets?customer=cus_1&kind=loyalty");
    expect(list.out).toContain("Flying Blue");
    expect(list.out).toContain("••7890");
    const got = await cli(["secrets", "get", "sec_1"], [{ body: secret }]);
    expect(got.calls[0]).toMatchObject({ method: "GET", url: "https://api.test/v1/secrets/sec_1" });
    const del = await cli(["secrets", "delete", "sec_1"], [{ status: 204 }]);
    expect(del.calls[0]).toMatchObject({ method: "DELETE", url: "https://api.test/v1/secrets/sec_1" });
    expect(del.code).toBe(0);
  });

  it("asks a customer to enter their own", async () => {
    const r = await cli(
      ["secrets", "request", "cus_1", "--kind", "contact", "--kind", "address", "--ttl", "3600"],
      [{ status: 201, body: { id: "sr_1", object: "secret_request", customer: "cus_1", kinds: ["contact", "address"], url: "https://openintents.io/secrets/abc", expires_at: "x" } }],
    );
    expect(r.calls[0]).toMatchObject({
      method: "POST",
      url: "https://api.test/v1/customers/cus_1/secret_requests",
      body: { kinds: ["contact", "address"], ttl_seconds: 3600 },
    });
    expect(r.out).toContain("https://openintents.io/secrets/abc");
  });
});

describe("openintents login", () => {
  it("saves a pasted key with owner-only permissions", async () => {
    const dir = await mkdtemp(join(tmpdir(), "oi-login-"));
    const configFile = join(dir, "cfg", "config.json");
    const opened: string[] = [];
    const r = await cli(["login"], [{ body: {} }], {
      configFile,
      stdin: async () => " oi_test_abc123 \n",
      openUrl: (u) => opened.push(u),
    });
    expect(r.code).toBe(0);
    expect(JSON.parse(await readFile(configFile, "utf8"))).toEqual({ apiKey: "oi_test_abc123" });
    expect((await stat(configFile)).mode & 0o777).toBe(0o600);
    expect(opened).toHaveLength(1);
    expect(r.out).not.toContain("oi_test_abc123");
  });

  it("rejects something that is not a key", async () => {
    const dir = await mkdtemp(join(tmpdir(), "oi-login-"));
    const r = await cli(["login"], [{ body: {} }], { configFile: join(dir, "config.json"), stdin: async () => "hunter2\n" });
    expect(r.code).toBe(1);
    expect(r.err).toMatch(/oi_live_/);
  });
});

describe("openintents intents", () => {
  it("still creates an intent and exits by its status", async () => {
    const r = await cli(["run", "flat white", "--max", "10"], [{ status: 201, body: { id: "int_1", object: "intent", status: "requires_payment", intent: "flat white", test: true, created_at: "x", payment: { total: 1000, fee: 50, currency: "usd", payment_url: "https://pay.example/1" } } }]);
    expect(r.calls[0]).toMatchObject({ method: "POST", url: "https://api.test/v1/intents", body: { intent: "flat white", max_amount: 1000 } });
    expect(r.code).toBe(2);
    expect(r.out).toContain("https://pay.example/1");
  });

  it("asks for a review with --review and prints the order to review", async () => {
    const review = JSON.parse(readFileSync(new URL("../../../spec/examples/intent-requires-review.json", import.meta.url), "utf8"));
    const r = await cli(["run", "snacks", "--review"], [{ status: 201, body: review }]);
    expect(r.calls[0]).toMatchObject({ method: "POST", url: "https://api.test/v1/intents", body: { intent: "snacks", review_before_order: true } });
    expect(r.out).toContain("order to review at blinkit.com:");
    expect(r.out).toContain("--input inp_review_1 --done to place it (or --value <change>, or --decline)");
  });

  it("prints errors as JSON with --json", async () => {
    const r = await cli(["status", "--json"]);
    expect(r.code).toBe(1);
    expect(JSON.parse(r.out)).toHaveProperty("error.message");
  });
});

describe("openintents status", () => {
  it("shows a payment's code to scan and answers it with --done", async () => {
    const body = {
      id: "int_1", object: "intent", status: "requires_input", intent: "a lamp", test: false, created_at: "x",
      input_request: {
        id: "inp_1", kind: "payment", shop: "blinkit.com", prompt: "Pay 249.00 INR", options: [], choices: [], fields: [],
        expects: "text", channel: null, length: null, url: "https://openintents.io/input/inp_1",
        image_url: "https://openintents.io/input/inp_1/code.jpg", expires_at: "x", attempts_left: 1,
      },
    };
    const r = await cli(["status", "int_1"], [{ body }]);
    expect(r.out).toContain("payment code to scan (theirs only): https://openintents.io/input/inp_1/code.jpg");
    expect(r.out).toContain("--input inp_1 --done");
  });

  it("status says a ride was requested, and why a cancelled intent may still be placed", async () => {
    const ride = {
      id: "int_1", object: "intent", status: "completed", intent: "an auto", test: false, created_at: "x",
      result: {
        merchant: "Uber", items: [], total: 18000, fee: 0, currency: "inr", confirmation: null, receipt_url: null,
        paid_with: "cash_on_delivery", kind: "ride", state: "requested",
      },
    };
    const r = await cli(["status", "int_1"], [{ body: ride }]);
    expect(r.out).toContain("ride requested");
    const cancelled = {
      id: "int_2", object: "intent", status: "cancelled", intent: "an auto", test: false, created_at: "x",
      error: { code: "may_be_placed", message: "cancelled after the order was placed at the shop" },
    };
    const c = await cli(["status", "int_2"], [{ body: cancelled }]);
    expect(c.out).toContain("error may_be_placed: cancelled after the order was placed at the shop");
  });
});
