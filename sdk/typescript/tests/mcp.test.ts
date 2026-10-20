import { PassThrough } from "node:stream";
import { describe, expect, it } from "vitest";

import { mcpUrl, runMcpBridge } from "../src/mcp.js";

type Json = Record<string, unknown>;
interface Seen {
  url: string;
  headers: Record<string, string>;
  method: string;
  body: Json | undefined;
}

const sse = (...messages: unknown[]) =>
  new Response(messages.map((m) => `data: ${JSON.stringify(m)}\n\n`).join(""), {
    headers: { "content-type": "text/event-stream" },
  });
const json = (body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), { headers: { "content-type": "application/json", ...headers } });

/** Runs the bridge on in-memory pipes against a stub /mcp, and collects what reached stdout. */
function harness(answer: (req: Seen) => Response | Promise<Response>) {
  const input = new PassThrough();
  const output = new PassThrough();
  const seen: Seen[] = [];
  const lines: Json[] = [];
  let text = "";
  const waiting: Array<() => void> = [];
  output.on("data", (chunk: Buffer) => {
    text += chunk.toString();
    for (let i = text.indexOf("\n"); i >= 0; i = text.indexOf("\n")) {
      lines.push(JSON.parse(text.slice(0, i)) as Json);
      text = text.slice(i + 1);
    }
    waiting.splice(0).forEach((w) => w());
  });
  const fetch = (async (url: URL | RequestInfo, init?: RequestInit) => {
    const req: Seen = {
      url: String(url),
      method: init?.method ?? "GET",
      headers: Object.fromEntries(Object.entries((init?.headers ?? {}) as Record<string, string>)),
      body: typeof init?.body === "string" ? (JSON.parse(init.body) as Json) : undefined,
    };
    seen.push(req);
    return answer(req);
  }) as typeof globalThis.fetch;
  const done = runMcpBridge({ url: "https://api.test/mcp", apiKey: "oi_test_k", input, output, fetch, log: () => {} });
  const send = (message: unknown) => input.write(JSON.stringify(message) + "\n");
  const until = async (n: number) => {
    while (lines.length < n) await new Promise<void>((r) => waiting.push(r));
  };
  return { send, until, lines, seen, end: () => input.end(), done };
}

describe("openintents mcp", () => {
  it("relays initialize, tools/list and a tool call with the key and the session", async () => {
    const h = harness((req) => {
      switch (req.body?.method) {
        case "initialize":
          return json({ jsonrpc: "2.0", id: req.body.id, result: { protocolVersion: "2025-11-25" } }, { "Mcp-Session-Id": "s1" });
        case "tools/list":
          return json({ jsonrpc: "2.0", id: req.body.id, result: { tools: [{ name: "create_intent" }] } });
        case "tools/call":
          return sse(
            { jsonrpc: "2.0", method: "notifications/progress", params: { progressToken: "p", progress: 1 } },
            { jsonrpc: "2.0", id: req.body.id, result: { content: [{ type: "text", text: "ok" }] } },
          );
        default:
          return new Response(null, { status: 202 });
      }
    });
    h.send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-11-25" } });
    await h.until(1);
    h.send({ jsonrpc: "2.0", method: "notifications/initialized" });
    h.send({ jsonrpc: "2.0", id: 2, method: "tools/list" });
    await h.until(2);
    h.send({ jsonrpc: "2.0", id: 3, method: "tools/call", params: { name: "create_intent", arguments: {} } });
    await h.until(4);
    h.end();
    await h.done;

    expect(h.lines[0]).toMatchObject({ id: 1, result: { protocolVersion: "2025-11-25" } });
    expect(h.lines[1]).toMatchObject({ id: 2, result: { tools: [{ name: "create_intent" }] } });
    expect(h.lines[2]).toMatchObject({ method: "notifications/progress" });
    expect(h.lines[3]).toMatchObject({ id: 3, result: { content: [{ text: "ok" }] } });

    const posts = h.seen.filter((r) => r.method === "POST");
    expect(posts).toHaveLength(4);
    for (const p of posts) {
      expect(p.url).toBe("https://api.test/mcp");
      expect(p.headers["Authorization"]).toBe("Bearer oi_test_k");
      expect(p.headers["Accept"]).toContain("text/event-stream");
    }
    expect(posts[0]!.headers["Mcp-Session-Id"]).toBeUndefined();
    expect(posts[1]!.headers["Mcp-Session-Id"]).toBe("s1");
    expect(posts[1]!.headers["MCP-Protocol-Version"]).toBe("2025-11-25");
    expect(posts[3]!.headers["Mcp-Session-Id"]).toBe("s1");
    // Closing stdin ends the server session.
    expect(h.seen.at(-1)).toMatchObject({ method: "DELETE" });
    expect(h.seen.at(-1)!.headers["Mcp-Session-Id"]).toBe("s1");
  });

  it("passes an elicitation request to the client and posts the client's answer back while the call still waits", async () => {
    let release!: (r: Response) => void;
    const h = harness((req) => {
      if (req.body?.method === "tools/call") {
        // The call's stream stays open: the elicitation goes out first, the result only after the reply arrived.
        const enc = new TextEncoder();
        let ctl!: ReadableStreamDefaultController<Uint8Array>;
        const stream = new ReadableStream<Uint8Array>({ start: (c) => (ctl = c) });
        const ask = { jsonrpc: "2.0", id: "elc_1", method: "elicitation/create", params: { message: "Which size?" } };
        ctl.enqueue(enc.encode(`: heartbeat\n\ndata: ${JSON.stringify(ask)}\n\n`));
        release = () => {
          ctl.enqueue(enc.encode(`data: ${JSON.stringify({ jsonrpc: "2.0", id: req.body!.id, result: { content: [] } })}\n\n`));
          ctl.close();
        };
        return new Response(stream, { headers: { "content-type": "text/event-stream" } });
      }
      return new Response(null, { status: 202 });
    });
    h.send({ jsonrpc: "2.0", id: 7, method: "tools/call", params: { name: "create_intent" } });
    await h.until(1);
    expect(h.lines[0]).toMatchObject({ id: "elc_1", method: "elicitation/create", params: { message: "Which size?" } });

    h.send({ jsonrpc: "2.0", id: "elc_1", result: { action: "accept", content: { size: "L" } } });
    while (!h.seen.some((r) => r.body?.id === "elc_1")) await new Promise((r) => setTimeout(r, 5));
    expect(h.seen.find((r) => r.body?.id === "elc_1")!.body).toMatchObject({ result: { action: "accept" } });

    release(new Response());
    await h.until(2);
    expect(h.lines[1]).toMatchObject({ id: 7, result: { content: [] } });
    h.end();
    await h.done;
  });

  it("answers a request the server refused with a JSON-RPC error carrying the api's message", async () => {
    const h = harness(
      () =>
        new Response(JSON.stringify({ error: { message: "Invalid API key." } }), {
          status: 401,
          headers: { "content-type": "application/json" },
        }),
    );
    h.send({ jsonrpc: "2.0", id: 9, method: "tools/list" });
    await h.until(1);
    expect(h.lines[0]).toMatchObject({ jsonrpc: "2.0", id: 9, error: { code: -32000, message: expect.stringContaining("Invalid API key.") } });
    h.end();
    await h.done;
  });

  it("answers a request when the server cannot be reached, and says nothing for a lost notification", async () => {
    const h = harness(() => {
      throw new TypeError("fetch failed");
    });
    h.send({ jsonrpc: "2.0", method: "notifications/initialized" });
    h.send({ jsonrpc: "2.0", id: 1, method: "tools/list" });
    await h.until(1);
    expect(h.lines[0]).toMatchObject({ id: 1, error: { message: expect.stringContaining("fetch failed") } });
    h.end();
    await h.done;
    expect(h.lines).toHaveLength(1);
  });

  it("derives the endpoint from the REST base URL", () => {
    expect(mcpUrl({})).toBe("https://api.openintents.io/mcp");
    expect(mcpUrl({ OPENINTENTS_BASE_URL: "http://localhost:8080/v1/" })).toBe("http://localhost:8080/mcp");
    expect(mcpUrl({ OPENINTENTS_MCP_URL: "https://x.test/mcp", OPENINTENTS_BASE_URL: "http://a/v1" })).toBe("https://x.test/mcp");
  });
});
