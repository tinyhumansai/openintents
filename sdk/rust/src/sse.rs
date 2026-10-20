//! A Server-Sent Events parser, by the rules of the HTML standard: lines end
//! in CRLF, LF or CR, comment lines (`:`, the stream's keep-alives) and
//! unknown fields are skipped, and a blank line ends a message.

/// A complete message: the last `id` the stream had sent by then, and its data.
pub(crate) type Message = (Option<String>, String);

#[derive(Debug, Default)]
pub(crate) struct SseParser {
    id: Option<String>,
    buffer: Vec<u8>,
    data: Vec<String>,
}

impl SseParser {
    /// The messages a chunk of the body completes; `None` means the body
    /// ended. Only messages that carry data are returned.
    pub(crate) fn feed(&mut self, chunk: Option<&[u8]>) -> Vec<Message> {
        let mut out = Vec::new();
        let Some(chunk) = chunk else {
            // A trailing CR still ends its line; a message without its
            // closing blank line is dropped, as the standard says.
            let rest = std::mem::take(&mut self.buffer);
            if let Some(line) = rest.strip_suffix(b"\r") {
                self.line(line, &mut out);
            }
            return out;
        };
        // Lines are split on bytes: CR and LF never occur inside a UTF-8
        // sequence, so a character cut between chunks is decoded whole.
        let mut buf = std::mem::take(&mut self.buffer);
        buf.extend_from_slice(chunk);
        let (mut start, mut i) = (0, 0);
        while i < buf.len() {
            let next = match buf[i] {
                b'\n' => i + 1,
                // Perhaps the first half of a CRLF cut in two: wait for more.
                b'\r' if i + 1 == buf.len() => break,
                b'\r' if buf[i + 1] == b'\n' => i + 2,
                b'\r' => i + 1,
                _ => {
                    i += 1;
                    continue;
                }
            };
            self.line(&buf[start..i], &mut out);
            (start, i) = (next, next);
        }
        buf.drain(..start);
        self.buffer = buf;
        out
    }

    fn line(&mut self, line: &[u8], out: &mut Vec<Message>) {
        if line.is_empty() {
            if !self.data.is_empty() {
                out.push((self.id.clone(), std::mem::take(&mut self.data).join("\n")));
            }
            return;
        }
        if line[0] == b':' {
            return;
        }
        let line = String::from_utf8_lossy(line);
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (&*line, ""),
        };
        match field {
            "data" => self.data.push(value.to_string()),
            "id" if !value.contains('\0') => self.id = Some(value.to_string()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(chunks: &[&[u8]]) -> Vec<Message> {
        let mut parser = SseParser::default();
        let mut out = Vec::new();
        for chunk in chunks {
            out.extend(parser.feed(Some(chunk)));
        }
        out.extend(parser.feed(None));
        out
    }

    #[test]
    fn reads_ids_and_data_and_skips_comments() {
        let got = feed_all(&[b": keep-alive\n\nid: evt_1\ndata: {\"a\":1}\n\nid:evt_2\ndata:x\ndata: y\nevent: z\n\n"]);
        assert_eq!(
            got,
            vec![
                (Some("evt_1".into()), "{\"a\":1}".into()),
                (Some("evt_2".into()), "x\ny".into())
            ]
        );
    }

    #[test]
    fn handles_crlf_and_cr_cut_anywhere() {
        let text = "id: evt_1\r\ndata: caf\u{e9}\r\n\r\nid: evt_2\rdata: b\r\r";
        let bytes = text.as_bytes();
        // Cut after every byte: inside the CRLFs and inside the two bytes of "é".
        let chunks: Vec<&[u8]> = bytes.chunks(1).collect();
        let got = feed_all(&chunks);
        assert_eq!(
            got,
            vec![
                (Some("evt_1".into()), "caf\u{e9}".into()),
                (Some("evt_2".into()), "b".into())
            ]
        );
    }

    #[test]
    fn drops_a_message_the_body_cut_short() {
        assert_eq!(feed_all(&[b"id: evt_1\ndata: x\n"]), vec![]);
    }
}
