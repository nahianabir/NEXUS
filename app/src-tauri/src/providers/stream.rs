//! Parsing helpers for SSE (`data: ...\n\n`) and Ollama NDJSON streaming.
//!
//! Both formats deliver a stream of small text frames. We yield a
//! `RawFrame` per frame so each provider can deserialize it.

use std::str;

use bytes::Bytes;

#[derive(Debug, Clone)]
pub enum RawFrame {
    /// SSE `data: <payload>`. The `<payload>` is included verbatim.
    SseData(String),
    /// SSE `data: [DONE]`. The provider signals end-of-stream this way.
    SseDone,
    /// A bare NDJSON line. (Ollama uses one JSON object per line.)
    NdjsonLine(String),
    /// Anything else: ignored, but kept so the caller can log.
    Other(String),
    /// Stream is finished (EOF on the response body).
    Eof,
}

pub struct FrameParser;

impl FrameParser {
    /// Parse a buffer + the new chunk. Yields the complete frames and
    /// returns the leftover buffer (frames that have not yet terminated).
    pub fn push(buf: &mut String, chunk: &Bytes) -> Vec<RawFrame> {
        buf.push_str(str::from_utf8(chunk).unwrap_or(""));
        let mut out = Vec::new();
        loop {
            let newline = buf.find('\n');
            let Some(idx) = newline else { break };
            let line: String = buf.drain(..=idx).collect();
            let line = line.trim_end_matches(['\r', '\n']).to_string();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("data:") {
                let rest = rest.trim();
                if rest == "[DONE]" {
                    out.push(RawFrame::SseDone);
                } else {
                    out.push(RawFrame::SseData(rest.to_string()));
                }
            } else if line.starts_with('{') || line.starts_with('[') {
                out.push(RawFrame::NdjsonLine(line));
            } else {
                out.push(RawFrame::Other(line));
            }
        }
        out
    }
}
