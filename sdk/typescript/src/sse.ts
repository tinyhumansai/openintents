/** A complete Server-Sent Events message, with the last `id` the stream had sent by then. */
export interface SseMessage {
  id: string | undefined;
  data: string;
}

/**
 * Splits a Server-Sent Events stream into messages, by the rules of the HTML
 * standard: lines end in CRLF, LF or CR, comment lines (`:`, the stream's
 * keep-alives) and unknown fields are skipped, and a blank line ends a
 * message. Only messages that carry `data` are returned.
 */
export class SseParser {
  private id: string | undefined;
  private buffer = "";
  private data: string[] = [];

  /** Feeds the next piece of the stream and returns the messages it completes. */
  push(text: string): SseMessage[] {
    this.buffer += text;
    const out: SseMessage[] = [];
    const eol = /\r\n|\r|\n/g;
    let start = 0;
    for (let m = eol.exec(this.buffer); m; m = eol.exec(this.buffer)) {
      // A CR at the very end may be the first half of a CRLF cut in two.
      if (m[0] === "\r" && eol.lastIndex === this.buffer.length) break;
      const message = this.line(this.buffer.slice(start, m.index));
      if (message) out.push(message);
      start = eol.lastIndex;
    }
    this.buffer = this.buffer.slice(start);
    return out;
  }

  /** The stream ended. A message without its closing blank line is dropped, as the standard says. */
  end(): SseMessage[] {
    const rest = this.buffer;
    this.buffer = "";
    if (!rest.endsWith("\r")) return [];
    const message = this.line(rest.slice(0, -1));
    return message ? [message] : [];
  }

  private line(line: string): SseMessage | undefined {
    if (line === "") return this.dispatch();
    if (line.startsWith(":")) return undefined;
    const colon = line.indexOf(":");
    const field = colon < 0 ? line : line.slice(0, colon);
    let value = colon < 0 ? "" : line.slice(colon + 1);
    if (value.startsWith(" ")) value = value.slice(1);
    if (field === "data") this.data.push(value);
    else if (field === "id" && !value.includes("\0")) this.id = value;
    return undefined;
  }

  private dispatch(): SseMessage | undefined {
    if (this.data.length === 0) return undefined;
    const data = this.data.join("\n");
    this.data = [];
    return { id: this.id, data };
  }
}
