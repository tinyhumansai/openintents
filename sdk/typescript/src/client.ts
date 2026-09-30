import { OpenIntentsError, WaitTimeoutError, type ApiErrorType } from "./errors.js";
import {
  FINAL_STATUSES,
  type Balance,
  type CreateIntentParams,
  type Intent,
  type IntentList,
  type ListIntentsParams,
} from "./types.js";
import { VERSION } from "./version.js";

export const DEFAULT_BASE_URL = "https://api.openintents.io/v1";

export interface ClientOptions {
  /** Defaults to the OPENINTENTS_API_KEY environment variable. */
  apiKey?: string;
  /** Defaults to OPENINTENTS_BASE_URL, then https://api.openintents.io/v1. */
  baseUrl?: string;
  /** Per-request timeout. Default 30s. */
  timeoutMs?: number;
  /** Retries for 429, 5xx and network errors. Default 2. */
  maxRetries?: number;
  /** Custom fetch (tests, proxies, older runtimes). */
  fetch?: typeof fetch;
}

export interface RequestOptions {
  /** Sent as Idempotency-Key. `create` and `cancel` generate one if omitted. */
  idempotencyKey?: string;
  signal?: AbortSignal;
}

export interface WaitOptions {
  /** Give up after this long. Default 5 minutes. */
  timeoutMs?: number;
  /** Time between polls. Default 2s. */
  intervalMs?: number;
  /**
   * Resolve when the intent needs payment (default true), so the caller can
   * hand `intent.payment.payment_url` to a person. Set false to keep waiting
   * until it is paid and final.
   */
  returnOnPayment?: boolean;
  signal?: AbortSignal;
}

function env(name: string): string | undefined {
  // `process` is absent in browsers and some edge runtimes.
  return typeof process !== "undefined" ? process.env?.[name] : undefined;
}

const sleep = (ms: number, signal?: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    const t = setTimeout(resolve, ms);
    signal?.addEventListener("abort", () => {
      clearTimeout(t);
      reject(signal.reason);
    });
  });

/**
 * Client for the OpenIntents agentic API.
 *
 * ```ts
 * const oi = new OpenIntents(); // reads OPENINTENTS_API_KEY
 * const intent = await oi.intents.create({ intent: "flat white, oat milk", max_amount: 1000 });
 * const settled = await oi.intents.wait(intent.id);
 * ```
 */
export class OpenIntents {
  readonly intents: Intents;
  readonly balance: BalanceResource;

  private readonly apiKey: string;
  private readonly baseUrl: string;
  private readonly timeoutMs: number;
  private readonly maxRetries: number;
  private readonly fetchImpl: typeof fetch;

  constructor(options: ClientOptions = {}) {
    const apiKey = options.apiKey ?? env("OPENINTENTS_API_KEY");
    if (!apiKey) {
      throw new Error("Missing API key: pass { apiKey } or set OPENINTENTS_API_KEY.");
    }
    this.apiKey = apiKey;
    this.baseUrl = (options.baseUrl ?? env("OPENINTENTS_BASE_URL") ?? DEFAULT_BASE_URL).replace(/\/+$/, "");
    this.timeoutMs = options.timeoutMs ?? 30_000;
    this.maxRetries = options.maxRetries ?? 2;
    this.fetchImpl = options.fetch ?? globalThis.fetch.bind(globalThis);
    this.intents = new Intents(this);
    this.balance = new BalanceResource(this);
  }

  /** @internal Makes a request, retrying 429/5xx/network errors with backoff. */
  async request<T>(
    method: "GET" | "POST",
    path: string,
    opts: { body?: unknown; query?: Record<string, string | number | undefined> } & RequestOptions = {},
  ): Promise<T> {
    const url = new URL(this.baseUrl + path);
    for (const [k, v] of Object.entries(opts.query ?? {})) {
      if (v !== undefined) url.searchParams.set(k, String(v));
    }
    const headers: Record<string, string> = {
      Authorization: `Bearer ${this.apiKey}`,
      Accept: "application/json",
      "User-Agent": `openintents-typescript/${VERSION}`,
    };
    if (opts.body !== undefined) headers["Content-Type"] = "application/json";
    if (opts.idempotencyKey) headers["Idempotency-Key"] = opts.idempotencyKey;

    for (let attempt = 0; ; attempt++) {
      const timeout = AbortSignal.timeout(this.timeoutMs);
      const signal = opts.signal ? AbortSignal.any([opts.signal, timeout]) : timeout;
      let res: Response;
      try {
        res = await this.fetchImpl(url, {
          method,
          headers,
          body: opts.body === undefined ? undefined : JSON.stringify(opts.body),
          signal,
        });
      } catch (err) {
        if (opts.signal?.aborted) throw err;
        if (attempt < this.maxRetries) {
          await sleep(backoff(attempt));
          continue;
        }
        throw new OpenIntentsError({
          message: `Network error: ${(err as Error).message}`,
          status: 0,
          type: "network",
        });
      }

      if (res.ok) return (await res.json()) as T;

      const retryable = res.status === 429 || res.status >= 500;
      if (retryable && attempt < this.maxRetries) {
        const after = Number(res.headers.get("retry-after"));
        await sleep(Number.isFinite(after) && after > 0 ? after * 1000 : backoff(attempt));
        continue;
      }
      throw await toError(res);
    }
  }
}

function backoff(attempt: number) {
  return Math.min(8000, 500 * 2 ** attempt) * (0.75 + Math.random() * 0.5);
}

async function toError(res: Response): Promise<OpenIntentsError> {
  let body: { error?: { type?: ApiErrorType; code?: string; message?: string; param?: string } } = {};
  try {
    body = (await res.json()) as typeof body;
  } catch {
    // Non-JSON error body; fall through to a generic message.
  }
  return new OpenIntentsError({
    message: body.error?.message ?? `HTTP ${res.status}`,
    status: res.status,
    type: body.error?.type ?? (res.status >= 500 ? "api_error" : "invalid_request"),
    code: body.error?.code,
    param: body.error?.param,
    requestId: res.headers.get("request-id") ?? undefined,
  });
}

class Intents {
  constructor(private readonly client: OpenIntents) {}

  /** Start an intent. Returns immediately, usually with `status: "queued"`. */
  create(params: CreateIntentParams, opts: RequestOptions = {}): Promise<Intent> {
    return this.client.request<Intent>("POST", "/intents", {
      body: params,
      ...opts,
      idempotencyKey: opts.idempotencyKey ?? crypto.randomUUID(),
    });
  }

  get(id: string, opts: RequestOptions = {}): Promise<Intent> {
    return this.client.request<Intent>("GET", `/intents/${encodeURIComponent(id)}`, opts);
  }

  list(params: ListIntentsParams = {}, opts: RequestOptions = {}): Promise<IntentList> {
    return this.client.request<IntentList>("GET", "/intents", { query: { ...params }, ...opts });
  }

  /** Iterate every intent matching `params`, following `next_cursor`. */
  async *listAll(params: Omit<ListIntentsParams, "cursor"> = {}): AsyncGenerator<Intent> {
    let cursor: string | undefined;
    do {
      const page = await this.list({ ...params, cursor });
      yield* page.data;
      cursor = page.next_cursor ?? undefined;
    } while (cursor);
  }

  /** Cancel an intent that hasn't paid yet. Cancelled intents are free. */
  cancel(id: string, opts: RequestOptions = {}): Promise<Intent> {
    return this.client.request<Intent>("POST", `/intents/${encodeURIComponent(id)}/cancel`, {
      ...opts,
      idempotencyKey: opts.idempotencyKey ?? crypto.randomUUID(),
    });
  }

  /**
   * Poll until the intent is final (`completed`, `failed`, `cancelled`) or, by
   * default, needs payment (`requires_payment`, with `payment.payment_url`).
   */
  async wait(id: string, opts: WaitOptions = {}): Promise<Intent> {
    const timeoutMs = opts.timeoutMs ?? 300_000;
    const intervalMs = opts.intervalMs ?? 2_000;
    const returnOnPayment = opts.returnOnPayment ?? true;
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const intent = await this.get(id, { signal: opts.signal });
      if (FINAL_STATUSES.includes(intent.status)) return intent;
      if (returnOnPayment && intent.status === "requires_payment") return intent;
      if (Date.now() + intervalMs > deadline) throw new WaitTimeoutError(id, timeoutMs);
      await sleep(intervalMs, opts.signal);
    }
  }
}

class BalanceResource {
  constructor(private readonly client: OpenIntents) {}

  /** The agent balance intents pay from, and this month's spend. */
  get(opts: RequestOptions = {}): Promise<Balance> {
    return this.client.request<Balance>("GET", "/balance", opts);
  }
}
