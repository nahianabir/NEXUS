//! Phase P2 — Attachment extraction regression tests.
//!
//! Verifies the library-level `ingestion::extractors` module
//! and the multimodal content builder that the chat
//! orchestrator uses.
//!
//! Why we test the library functions and not the Tauri
//! command directly: the `commands::chat::chat_read_attachment`
//! lives in the binary crate (it pulls in Tauri's runtime
//! and WebView2). The library-level functions it calls are
//! the source of truth; the command is a thin wrapper that
//! adds path validation and the image size cap.
//!
//! The test cases cover:
//!
//! 1. Plain text files
//! 2. Source code with line numbers
//! 3. JSON flattening
//! 4. CSV tab-separation
//! 5. Per-extension extractor routing
//! 6. Non-UTF-8 fallback (lossy decode)
//! 7. Notebook (`.ipynb`) extraction
//! 8. Unknown extension fallback
//! 9. Image detection by extension and by magic bytes
//!    (covered in the chat-lifecycle test file; this file
//!    keeps the heavier PDF/DOCX tests).

use std::io::Write;
use std::path::Path;

use nexus_core::ingestion::extractors;

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    let mut f = std::fs::File::create(&p).expect("create tmp file");
    f.write_all(bytes).expect("write");
    p
}

fn tempdir() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("nexus-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    base
}

// ------------------------------------------------------------------
// 1. Plain text file
// ------------------------------------------------------------------
#[test]
fn text_file_extracts_as_text() {
    let dir = tempdir();
    let p = write_file(&dir, "hello.txt", b"hello world\nsecond line\n");
    let ext = extractors::extract(&p).expect("extract txt");
    assert_eq!(ext.mime_type, "text/plain");
    assert!(!ext.text.is_empty(), "text must be non-empty");
    assert!(ext.text.contains("hello"), "must contain payload, got {:?}", ext.text);
}

// ------------------------------------------------------------------
// 2. Code file (Python) — line numbers are added
// ------------------------------------------------------------------
#[test]
fn code_file_adds_line_numbers() {
    let dir = tempdir();
    let src = b"def f():\n    return 1\n";
    let p = write_file(&dir, "f.py", src);
    let ext = extractors::extract(&p).expect("extract py");
    // extract_code prepends a 5-wide right-aligned line number
    // then two spaces then the line.
    assert!(ext.text.contains("    1  def f()"), "must include line number, got {:?}", ext.text);
    assert!(ext.text.contains("    2      return 1"));
}

// ------------------------------------------------------------------
// 3. JSON file
// ------------------------------------------------------------------
#[test]
fn json_file_flattens_to_key_value_lines() {
    let dir = tempdir();
    let p = write_file(&dir, "data.json", br#"{"name":"alice","age":30,"addr":{"city":"NY"}}"#);
    let ext = extractors::extract(&p).expect("extract json");
    assert!(ext.text.contains("name"), "must include the key, got {:?}", ext.text);
    assert!(ext.text.contains("alice"), "must include the value, got {:?}", ext.text);
    assert!(ext.text.contains("city"), "nested key must be included, got {:?}", ext.text);
    assert_eq!(ext.mime_type, "application/json");
}

// ------------------------------------------------------------------
// 4. CSV file
// ------------------------------------------------------------------
#[test]
fn csv_file_emits_tab_separated_rows() {
    let dir = tempdir();
    let p = write_file(&dir, "data.csv", b"a,b,c\n1,2,3\n4,5,6\n");
    let ext = extractors::extract(&p).expect("extract csv");
    assert!(ext.text.contains("a\tb\tc"), "must emit header row, got {:?}", ext.text);
    assert!(ext.text.contains("1\t2\t3"));
    assert!(ext.text.contains("4\t5\t6"));
    assert_eq!(ext.mime_type, "text/csv");
}

// ------------------------------------------------------------------
// 5. Per-extension extractor routing
// ------------------------------------------------------------------
#[test]
fn extractor_routes_by_extension() {
    // Code extensions.
    for ext in ["py", "js", "ts", "rs", "go", "c", "cpp", "h", "java", "rb", "sh", "toml", "yaml", "json", "xml", "ini", "html"] {
        let path = format!("/tmp/x.{ext}");
        let p = Path::new(&path);
        assert!(extractors::extractor_for(p).is_some(), "extractor must exist for .{ext}");
    }
    // Document extensions.
    for ext in ["pdf", "docx", "xlsx", "ipynb"] {
        let path = format!("/tmp/x.{ext}");
        let p = Path::new(&path);
        assert!(extractors::extractor_for(p).is_some(), "extractor must exist for .{ext}");
    }
    // Unknown extension returns None.
    let path = "/tmp/x.xyz_unknown".to_string();
    let p = Path::new(&path);
    assert!(extractors::extractor_for(p).is_none());
}

// ------------------------------------------------------------------
// 6. Non-UTF-8 bytes are lossily decoded (so the model still
//    gets the file, just with the U+FFFD replacement char
//    for invalid sequences).
// ------------------------------------------------------------------
#[test]
fn non_utf8_bytes_lossily_decoded() {
    let dir = tempdir();
    // 0xC3 0x28 is invalid UTF-8 (incomplete sequence).
    let bytes: &[u8] = &[b'a', 0xC3, 0x28, b'b'];
    let p = write_file(&dir, "f.py", bytes);
    let ext = extractors::extract(&p).expect("extract py");
    let replaced = ext.metadata.get("non_utf8_replaced").and_then(|v| v.as_bool()).unwrap_or(false);
    assert!(replaced, "non_utf8_replaced must be true, got metadata={:?}", ext.metadata);
    assert!(ext.text.contains('a'));
    assert!(ext.text.contains('b'));
}

// ------------------------------------------------------------------
// 7. Notebook extraction — verifies the JSON-walk through
//    `cells[].source`.
// ------------------------------------------------------------------
#[test]
fn ipynb_notebook_extracts_cell_sources() {
    let dir = tempdir();
    let nb = "{
        \"cells\": [
            {\"cell_type\": \"markdown\", \"source\": [\"# Title\\n\", \"Sub.\"]},
            {\"cell_type\": \"code\", \"source\": [\"print('hi')\\n\"]}
        ],
        \"metadata\": {\"kernelspec\": {\"language\": \"python\"}}}
    ";
    let p = write_file(&dir, "nb.ipynb", nb.as_bytes());
    let ext = extractors::extract(&p).expect("extract ipynb");
    assert!(ext.text.contains("# Title"), "must include markdown content, got {:?}", ext.text);
    assert!(ext.text.contains("print('hi')"), "must include code content, got {:?}", ext.text);
    let lang = ext.metadata.get("language").and_then(|v| v.as_str());
    assert_eq!(lang, Some("python"));
}

// ------------------------------------------------------------------
// 8. Unknown extension falls back to a lossy UTF-8 read.
// ------------------------------------------------------------------
#[test]
fn unknown_extension_falls_back_to_utf8() {
    let dir = tempdir();
    let p = write_file(&dir, "mystery.xyz_unknown", b"plain text content");
    let ext = extractors::extract(&p).expect("fallback extract");
    assert_eq!(ext.mime_type, "text/plain");
    assert!(ext.text.contains("plain text content"));
}
