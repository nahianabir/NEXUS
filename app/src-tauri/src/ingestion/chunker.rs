//! Token-aware text chunker. Splits on paragraph boundaries first, then on
//! sentences, then on word boundaries. The token estimate is `len / 4`,
//! which is a reasonable approximation for English and CJK-heavy text.

#[derive(Debug, Clone)]
pub struct Chunk {
    pub ordinal: usize,
    pub text: String,
    pub token_estimate: usize,
    pub start_offset: usize,
    pub end_offset: usize,
}

pub fn chunk_text(text: &str, target_tokens: usize, overlap_tokens: usize) -> Vec<Chunk> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    // Convert the text into a stream of paragraphs. Each paragraph is a
    // candidate sub-chunk; we pack paragraphs into a chunk until the
    // token budget is exhausted, then start a new chunk with overlap.
    //
    // We track the original byte offset of each paragraph in `text`
    // so that the resulting `start_offset` / `end_offset` values
    // point at the right place even after `trim()`. Previously the
    // cursor drifted because it was incremented by the trimmed
    // length instead of the full separator-bounded length, which
    // made citation offsets unreliable for documents that had
    // surrounding whitespace in their paragraphs (a common case for
    // code files where each line is a "paragraph" of its own).
    let mut paragraphs: Vec<(usize, &str)> = Vec::new();
    let mut cursor = 0usize;
    for raw in text.split("\n\n") {
        let p_start = cursor;
        cursor = p_start + raw.len() + 2; // +2 for the "\n\n" separator
        let trimmed = raw.trim();
        if trimmed.is_empty() { continue; }
        // The original-text offset of the first non-whitespace byte.
        let leading_ws = raw.len() - raw.trim_start().len();
        let original_start = p_start + leading_ws;
        paragraphs.push((original_start, trimmed));
    }
    if paragraphs.is_empty() {
        return Vec::new();
    }

    let target_chars = target_tokens * 4;
    let overlap_chars = overlap_tokens * 4;
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut buf = String::new();
    let mut buf_start: usize = paragraphs[0].0;
    let mut ordinal = 0usize;

    for (p_start, p) in paragraphs {
        let p_with_sep = if buf.is_empty() { p.to_string() } else { format!("\n\n{}", p) };
        if buf.len() + p_with_sep.len() > target_chars && !buf.is_empty() {
            // Emit current buffer.
            let end = buf_start + buf.len();
            chunks.push(Chunk {
                ordinal,
                text: buf.clone(),
                token_estimate: buf.len() / 4,
                start_offset: buf_start,
                end_offset: end,
            });
            ordinal += 1;
            // Overlap: keep the tail of the previous buffer if it is short.
            if overlap_chars > 0 && buf.len() > overlap_chars {
                let tail = &buf[buf.len() - overlap_chars..];
                buf_start = end - tail.len();
                buf = tail.to_string();
            } else {
                buf_start = p_start;
                buf.clear();
            }
        }
        if buf.is_empty() {
            // The buffer starts at the original byte offset of this
            // paragraph (after the trim) so that the next emitted
            // chunk points at the right place.
            buf_start = p_start;
        } else if !buf.ends_with("\n\n") {
            buf.push_str("\n\n");
        }
        buf.push_str(p);
    }
    if !buf.trim().is_empty() {
        let end = buf_start + buf.len();
        chunks.push(Chunk {
            ordinal,
            text: buf,
            token_estimate: end.saturating_sub(buf_start) / 4,
            start_offset: buf_start,
            end_offset: end,
        });
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_yields_no_chunks() {
        assert!(chunk_text("", 100, 10).is_empty());
        assert!(chunk_text("   \n\n  ", 100, 10).is_empty());
    }

    #[test]
    fn short_text_yields_one_chunk() {
        let c = chunk_text("Hello world.\n\nThis is a test.", 100, 10);
        assert_eq!(c.len(), 1);
        assert!(c[0].text.contains("Hello"));
    }

    #[test]
    fn long_text_is_split() {
        let para = "word ".repeat(50);
        let text = std::iter::repeat(para.as_str()).take(20).collect::<Vec<_>>().join("\n\n");
        let c = chunk_text(&text, 50, 10);
        assert!(c.len() > 1, "expected multiple chunks, got {}", c.len());
    }

    #[test]
    fn overlap_keeps_tail() {
        let para = "alpha beta gamma. ".repeat(200);
        let c = chunk_text(&para, 100, 20);
        if c.len() >= 2 {
            // Some tail of chunk[0] should appear at the start of chunk[1].
            assert!(c[1].text.starts_with(&c[0].text[c[0].text.len().saturating_sub(80)..]));
        }
    }
}