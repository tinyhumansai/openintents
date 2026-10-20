/**
 * `openintents mcp`: a local stdio MCP server that relays JSON-RPC to the
 * hosted streamable-HTTP `/mcp` endpoint, for clients that only speak stdio.
 *
 * Every line the client writes is POSTed to the endpoint with the user's key.
 * Whatever the endpoint answers goes back to the client as one line each: a
 * plain JSON reply as it is, and a Server-Sent Events reply message by
 * message, so progress notifications and the server's own requests
 * (`elicitation/create`) reach the client while the call is still running.
 * The client's answer to such a request is just another line, relayed the
 * same way, so a call that waits on a person is not held up by the bridge.
 */
import { createInterface } from "node:readline";
import type { Readable, Writable } from "node:stream";

import { DEFAULT_BASE_URL } from "./client.js";
import { SseParser } from "./sse.js";
import { VERSION } from "./version.js";

type Json = Record<string, unknown>;

const DEFAULT_MCP_URL = DEFAULT_BASE_URL.replace(/\/v1$/, "/mcp");
/** JSON-RPC's range for errors the implementation defines. */
const BRIDGE_ERROR = -32000;
const PARSE_ERROR = -32700;

/** Where the hosted MCP endpoint is: `OPENINTENTS_MCP_URL`, else beside `OPENINTENTS_BASE_URL`, else the hosted one. */
export function mcpUrl(env: Record<string, string | undefined> = process.env): string {
  if (env.OPENINTENTS_MCP_URL) return env.OPENINTENTS_MCP_URL;
  if (env.OPENINTENTS_BASE_URL) return env.OPENINTENTS_BASE_URL.replace(/\/+$/, "").replace(/\/v1$/, "") + "/mcp";
  return DEFAULT_MCP_URL;
}

export interface McpBridgeOptions {
  url: string;
  apiKey: string;
  /** The client's messages, one JSON value per line. Defaults to stdin. */
  input?: Readable;
  /** Where messages for the client go, one per line. Defaults to stdout. */
  output?: Writable;
  /** Diagnostics for the person running the client. Defaults to stderr. */
  log?: (line: string) => void;
  fetch?: typeof fetch;
}

const isObject = (v: unknown): v is Json => typeof v === "object" && v !== null && !Array.isArray(v);

/** Relays until the input ends and every call in flight has been answered, then closes the server session. */
export async function runMcpBridge(options: McpBridgeOptions): Promise<void> {
  const input = options.input ?? process.stdin;
  const output = options.output ?? process.stdout;
  const log = options.log ?? ((line: string) => process.stderr.write(`openintents mcp: ${line}\n`));
  const doFetch = options.fetch ?? globalThis.fetch;

  let sessionId: string | undefined;
  let protocolVersion: string | undefined;

  const emit = (message: unknown) => {
    output.write(JSON.stringify(message) + "\n");
  };
  const fail = (id: unknown, message: string) =>
    emit({ jsonrpc: "2.0", id, error: { code: BRIDGE_ERROR, message } });

  const headers = (): Record<string, string> => ({
    "Content-Type": "application/json",
    Accept: "application/json, text/event-stream",
    Authorization: `Bearer ${options.apiKey}`,
    "User-Agent": `openintents-mcp-bridge/${VERSION}`,
    ...(sessionId ? { "Mcp-Session-Id": sessionId } : {}),
    ...(protocolVersion ? { "MCP-Protocol-Version": protocolVersion } : {}),
  });

  /** Passes one message from the server on, and learns the protocol version from `initialize`'s answer. */
  const deliver = (message: unknown, request: Json) => {
    if (request.method === "initialize" && isObject(message) && isObject(message.result)) {
      const v = message.result.protocolVersion;
      if (typeof v === "string") protocolVersion = v;
    }
    emit(message);
  };

  const relay = async (message: Json) => {
    const isRequest = typeof message.method === "string" && message.id !== undefined;
    const answered = new Set<unknown>();
    const say = (m: unknown) => {
      if (isObject(m) && m.method === undefined && m.id !== undefined) answered.add(m.id);
      deliver(m, message);
    };
    try {
      const res = await doFetch(options.url, { method: "POST", headers: headers(), body: JSON.stringify(message) });
      sessionId = res.headers.get("Mcp-Session-Id") ?? sessionId;
      const type = res.headers.get("content-type") ?? "";
      if (res.ok && type.includes("text/event-stream") && res.body) {
        const parser = new SseParser();
        const decoder = new TextDecoder();
        const take = (text: string) => {
          for (const m of parser.push(text)) {
            try {
              say(JSON.parse(m.data));
            } catch {
              log("ignored a server message that is not JSON");
            }
          }
        };
        for await (const chunk of res.body as unknown as AsyncIterable<Uint8Array>) take(decoder.decode(chunk, { stream: true }));
        take(decoder.decode());
        if (isRequest && !answered.has(message.id)) fail(message.id, "The server closed the stream without answering.");
        return;
      }
      const text = await res.text();
      if (res.ok) {
        if (text.trim() !== "") say(JSON.parse(text));
        else if (isRequest && res.status !== 202) fail(message.id, "The server sent no answer.");
        return;
      }
      // A failure: pass a JSON-RPC error the server wrote as it is, else say what the API said.
      let body: unknown;
      try {
        body = JSON.parse(text);
      } catch {
        body = undefined;
      }
      if (isRequest && isObject(body) && body.jsonrpc === "2.0" && body.error !== undefined) return emit(body);
      const detail = isObject(body) && isObject(body.error) && typeof body.error.message === "string" ? body.error.message : text.slice(0, 200);
      const reason = `HTTP ${res.status}${detail ? `: ${detail}` : ""}`;
      if (isRequest) fail(message.id, reason);
      else log(reason);
    } catch (err) {
      const reason = (err as Error).message || String(err);
      if (isRequest && !answered.has(message.id)) fail(message.id, reason);
      else log(reason);
    }
  };

  const inFlight = new Set<Promise<void>>();
  const lines = createInterface({ input, crlfDelay: Infinity });
  for await (const line of lines) {
    if (line.trim() === "") continue;
    let message: unknown;
    try {
      message = JSON.parse(line);
    } catch {
      emit({ jsonrpc: "2.0", id: null, error: { code: PARSE_ERROR, message: "Parse error" } });
      continue;
    }
    if (!isObject(message) && !Array.isArray(message)) {
      emit({ jsonrpc: "2.0", id: null, error: { code: PARSE_ERROR, message: "Expected a JSON-RPC message" } });
      continue;
    }
    // Concurrently: a call that holds its stream open must not hold up the answer to its own elicitation.
    const task: Promise<void> = relay(message as Json).finally(() => inFlight.delete(task));
    inFlight.add(task);
  }
  while (inFlight.size > 0) await Promise.all([...inFlight]);

  if (sessionId) {
    try {
      await doFetch(options.url, { method: "DELETE", headers: headers() });
    } catch {
      // The session expires on its own.
    }
  }
}
