import { describe, expect, it } from "vitest";

import { signWebhook, verifyWebhook, WebhookVerificationError } from "../src/index.js";

const body = JSON.stringify({ id: "evt_1", type: "intent.completed", created_at: "x", data: { id: "int_1" } });
const secret = "whsec_test";

describe("webhooks", () => {
  it("accepts a valid signature", async () => {
    const header = await signWebhook(body, secret);
    const event = await verifyWebhook(body, header, secret);
    expect(event.type).toBe("intent.completed");
  });

  it("rejects a tampered body", async () => {
    const header = await signWebhook(body, secret);
    await expect(verifyWebhook(body + " ", header, secret)).rejects.toBeInstanceOf(WebhookVerificationError);
  });

  it("rejects the wrong secret", async () => {
    const header = await signWebhook(body, "whsec_other");
    await expect(verifyWebhook(body, header, secret)).rejects.toThrow(/mismatch/);
  });

  it("rejects old timestamps", async () => {
    const header = await signWebhook(body, secret, Math.floor(Date.now() / 1000) - 3600);
    await expect(verifyWebhook(body, header, secret)).rejects.toThrow(/tolerance/);
  });

  it("rejects a missing header", async () => {
    await expect(verifyWebhook(body, null, secret)).rejects.toThrow(/Missing/);
  });

  it("verifies session events, which carry a session", async () => {
    const raw = JSON.stringify({
      id: "evt_2",
      type: "session.closed",
      created_at: "x",
      data: { id: "ses_1", object: "session", status: "closed", close_reason: "idle" },
    });
    const event = await verifyWebhook(raw, await signWebhook(raw, secret), secret);
    if (event.type !== "session.closed") throw new Error(`unexpected ${event.type}`);
    expect(event.data.status).toBe("closed");
    expect(event.data.close_reason).toBe("idle");
  });

  it("matches the shared test vector", async () => {
    // The Python and Rust suites assert the same value.
    expect(await signWebhook('{"id":"evt_1"}', "whsec_x", 1_700_000_000)).toBe(
      "t=1700000000,v1=366e40c480fe3a9e6a03609dca079cea86d77ae82d5a56b5348208247317a8d1",
    );
  });
});

describe("webhooks during a secret rotation", () => {
  const t = Math.floor(Date.now() / 1000);
  const v1 = async (s: string) => (await signWebhook(body, s, t)).split(",")[1];
  const both = async () => `t=${t},${await v1("whsec_new")},${await v1("whsec_old")}`;

  it("accepts a header with a v1 for each secret, whichever secret the receiver holds", async () => {
    expect((await verifyWebhook(body, await both(), "whsec_new")).type).toBe("intent.completed");
    expect((await verifyWebhook(body, await both(), "whsec_old")).type).toBe("intent.completed");
  });

  it("still rejects a header none of whose v1 values match", async () => {
    await expect(verifyWebhook(body, await both(), "whsec_other")).rejects.toThrow(/mismatch/);
  });

  it("verifies the new event types", async () => {
    const raw = JSON.stringify({
      id: "evt_3",
      type: "sign_in.completed",
      created_at: "x",
      data: { id: "sgn_1", object: "sign_in", status: "done", shop: "amazon.com" },
    });
    const event = await verifyWebhook(raw, await signWebhook(raw, secret), secret);
    if (event.type !== "sign_in.completed") throw new Error(`unexpected ${event.type}`);
    expect(event.data.shop).toBe("amazon.com");
  });
});
