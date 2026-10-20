import type { WebhookEvent } from "./types.js";

/** Default tolerance for the signature timestamp, in seconds. */
export const DEFAULT_TOLERANCE_S = 300;

export class WebhookVerificationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WebhookVerificationError";
  }
}

const encoder = new TextEncoder();

async function hmacHex(secret: string, payload: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign("HMAC", key, encoder.encode(payload));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** Constant-time comparison of two hex strings. */
function safeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

/**
 * Verify an `OpenIntents-Signature` header (`t=<unix>,v1=<hex>`, with one `v1`
 * for each secret that signs while an endpoint's secret is being rotated) against the
 * raw request body and return the parsed event. Throws
 * WebhookVerificationError if the signature is missing, wrong or too old.
 * Pass the body exactly as received, before any JSON parsing.
 */
export async function verifyWebhook(
  rawBody: string,
  signatureHeader: string | null | undefined,
  secret: string,
  { toleranceS = DEFAULT_TOLERANCE_S, now = Date.now() }: { toleranceS?: number; now?: number } = {},
): Promise<WebhookEvent> {
  if (!signatureHeader) throw new WebhookVerificationError("Missing OpenIntents-Signature header");
  // After a secret rotation the header holds a `v1` for each secret that still signs.
  let tRaw: string | undefined;
  const v1s: string[] = [];
  for (const part of signatureHeader.split(",")) {
    const [k, v] = part.trim().split("=", 2);
    if (!k || !v) continue;
    if (k === "t") tRaw = v;
    else if (k === "v1") v1s.push(v.toLowerCase());
  }
  const t = Number(tRaw);
  if (!Number.isFinite(t) || v1s.length === 0) throw new WebhookVerificationError("Malformed OpenIntents-Signature header");
  if (Math.abs(now / 1000 - t) > toleranceS) throw new WebhookVerificationError("Signature timestamp outside tolerance");

  const expected = await hmacHex(secret, `${t}.${rawBody}`);
  if (!v1s.map((v1) => safeEqual(expected, v1)).includes(true)) throw new WebhookVerificationError("Signature mismatch");
  return JSON.parse(rawBody) as WebhookEvent;
}

/** Build a signature header, for tests and local webhook replays. */
export async function signWebhook(rawBody: string, secret: string, t = Math.floor(Date.now() / 1000)): Promise<string> {
  return `t=${t},v1=${await hmacHex(secret, `${t}.${rawBody}`)}`;
}
