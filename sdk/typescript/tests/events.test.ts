import { describe, expect, it, vi } from "vitest";

import { OpenIntents, OpenIntentsError, type Event } from "../src/index.js";

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

const make = (f: typeof fetch, maxRetries = 0) =>
  new OpenIntents({ apiKey: "oi_test_x", baseUrl: "https://api.test/v1", fetch: f, maxRetries });

const header = (call: { init: RequestInit } | undefined, name: string) =>
  (call?.init.headers as Record<string, string>)[name];

const event = (id: string, type = "status", data: Record<string, unknown> = { status: "running" }): Event => ({
  id,
  object: "event",
  type,
  created_at: "2026-10-01T08:30:00Z",
  data,
});

/** An SSE frame as the API sends it. */
const frame = (e: Event) => `id: ${e.id}\ndata: ${JSON.stringify(e)}\n\n`;

const sse = (text: string): Reply => ({ raw: text, headers: { "content-type": "text/event-stream" } });

/** A body that arrives in the given pieces and then stays open (or closes). */
function chunked(pieces: Array<string | Uint8Array>, { close = true, onCancel = () => {} } = {}) {
  const encoder = new TextEncoder();
  return new ReadableStream<Uint8Array>({
    start(controller) {
      for (const piece of pieces) controller.enqueue(typeof piece === "string" ? encoder.encode(piece) : piece);
      if (close) controller.close();
    },
    cancel: onCancel,
  });
}

async function collect(events: AsyncIterable<Event>): Promise<string[]> {
  const ids: string[] = [];
  for await (const e of events) ids.push(e.id);
  return ids;
}

describe("events", () => {
  it("lists a page of an intent's and a session's events", async () => {
    const page = { object: "list", data: [event("evt_2")], next_cursor: "evt_2" };
    const { fetch, calls } = mockFetch({ body: page }, { body: page });
    const oi = make(fetch);
    const got = await oi.intents.listEvents("int_1", { limit: 1, cursor: "evt_1" });
    expect(got.data[0]!.id).toBe("evt_2");
    expect(got.next_cursor).toBe("evt_2");
    expect(calls[0]!.url).toBe("https://api.test/v1/intents/int_1/events?limit=1&cursor=evt_1");
    expect(header(calls[0], "Accept")).toBe("application/json");
    await oi.sessions.listEvents("ses_1");
    expect(calls[1]!.url).toBe("https://api.test/v1/sessions/ses_1/events");
  });

  it("streams events, skips keep-alives, and resumes after the last one until a 204", async () => {
    const { fetch, calls } = mockFetch(
      sse(`: keep-alive\n\n${frame(event("evt_1"))}${frame(event("evt_2", "step", { action: "open" }))}`),
      sse(`${frame(event("evt_3", "checkout", { merchant: "Blue Bottle", total: 550, currency: "usd" }))}`),
      { status: 204, raw: null },
    );
    const got: Event[] = [];
    for await (const e of make(fetch).intents.streamEvents("int_1")) got.push(e);

    expect(got.map((e) => e.id)).toEqual(["evt_1", "evt_2", "evt_3"]);
    expect(got[1]!.data).toEqual({ action: "open" });
    expect(calls[0]!.url).toBe("https://api.test/v1/intents/int_1/events");
    expect(header(calls[0], "Accept")).toBe("text/event-stream");
    expect(header(calls[0], "Last-Event-ID")).toBeUndefined();
    // The API closed the stream: it is resumed after the last event, until it says the timeline is over.
    expect(header(calls[1], "Last-Event-ID")).toBe("evt_2");
    expect(header(calls[2], "Last-Event-ID")).toBe("evt_3");
    expect(calls).toHaveLength(3);
  });

  it("parses events cut anywhere, with CRLF or CR line ends", async () => {
    const one = JSON.stringify(event("evt_1", "page", { url: "https://shop.example/", title: "Café" }));
    const bytes = new TextEncoder().encode(`id: evt_1\r\ndata: ${one}\r\n\r\n`);
    // Cut inside the CRLF, and inside the two bytes of "é".
    const cr = bytes.indexOf(13);
    const e = bytes.indexOf(0xc3);
    const two = `id:evt_2\rdata:${JSON.stringify(event("evt_2"))}\r\r`;
    const { fetch } = mockFetch(
      { raw: chunked([bytes.slice(0, cr + 1), bytes.slice(cr + 1, e + 1), bytes.slice(e + 1), two]) },
      { status: 204, raw: null },
    );
    const got: Event[] = [];
    for await (const ev of make(fetch).sessions.streamEvents("ses_1")) got.push(ev);
    expect(got.map((ev) => ev.id)).toEqual(["evt_1", "evt_2"]);
    expect(got[0]!.data.title).toBe("Café");
  });

  it("starts after lastEventId and retries a 429", async () => {
    const { fetch, calls } = mockFetch(
      {
        status: 429,
        headers: { "retry-after": "0.01" },
        body: { error: { type: "rate_limit", code: "too_many_streams", message: "Too many open streams." } },
      },
      sse(frame(event("evt_10"))),
      { status: 204, raw: null },
    );
    const ids = await collect(make(fetch, 1).sessions.streamEvents("ses_1", { lastEventId: "evt_9" }));
    expect(ids).toEqual(["evt_10"]);
    expect(calls.map((c) => header(c, "Last-Event-ID"))).toEqual(["evt_9", "evt_9", "evt_10"]);
  });

  it("ends at once when the timeline is already over", async () => {
    const { fetch, calls } = mockFetch({ status: 204, raw: null });
    expect(await collect(make(fetch).intents.streamEvents("int_1", { lastEventId: "evt_9" }))).toEqual([]);
    expect(calls).toHaveLength(1);
  });

  it("throws errors that are not worth retrying", async () => {
    const { fetch } = mockFetch({ status: 404, body: { error: { type: "not_found", message: "No such intent." } } });
    const err = await collect(make(fetch, 2).intents.streamEvents("int_x")).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(OpenIntentsError);
    expect(err).toMatchObject({ status: 404, type: "not_found" });
  });

  it("gives up on a stream that keeps closing empty", async () => {
    const { fetch, calls } = mockFetch(sse(""));
    const err = await collect(make(fetch).intents.streamEvents("int_1")).catch((e: unknown) => e);
    expect(err).toMatchObject({ status: 0, type: "network" });
    expect(calls).toHaveLength(1);
  });

  it("closes the connection when the caller stops early", async () => {
    const onCancel = vi.fn();
    const { fetch } = mockFetch({ raw: chunked([frame(event("evt_1")), frame(event("evt_2"))], { close: false, onCancel }) });
    for await (const e of make(fetch).intents.streamEvents("int_1")) {
      expect(e.id).toBe("evt_1");
      break;
    }
    expect(onCancel).toHaveBeenCalled();
  });

  it("stops when its signal aborts", async () => {
    const ctrl = new AbortController();
    const { fetch } = mockFetch({ raw: chunked([frame(event("evt_1"))], { close: false }) });
    const ids: string[] = [];
    const err = await (async () => {
      for await (const e of make(fetch).intents.streamEvents("int_1", { signal: ctrl.signal })) {
        ids.push(e.id);
        ctrl.abort(new Error("enough"));
      }
    })().catch((e: unknown) => e);
    expect(ids).toEqual(["evt_1"]);
    expect((err as Error).message).toBe("enough");
  });
});
