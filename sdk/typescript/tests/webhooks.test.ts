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

  it("matches the shared test vector", async () => {
    // The Python and Rust suites assert the same value.
    expect(await signWebhook('{"id":"evt_1"}', "whsec_x", 1_700_000_000)).toBe(
      "t=1700000000,v1=366e40c480fe3a9e6a03609dca079cea86d77ae82d5a56b5348208247317a8d1",
    );
  });
});
