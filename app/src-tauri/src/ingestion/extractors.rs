//! Per-file-type text extractors. Each returns `(text, metadata)` where
//! `metadata` is a JSON string the caller can persist on the `documents` row.

use std::path::Path;

use crate::error::{AppError, AppResult};

pub struct Extraction {
    pub text: String,
    pub mime_type: String,
    pub metadata: serde_json::Value,
}

pub fn extractor_for(path: &Path) -> Option<fn(&Path) -> AppResult<Extraction>> {
    let ext = path.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("txt") | Some("md") | Some("markdown") | Some("rst") | Some("log") | Some("text") => Some(extract_text),
        Some("json") => Some(extract_json),
        Some("csv") | Some("tsv") => Some(extract_csv),
        Some("html") | Some("htm") | Some("xhtml") => Some(extract_html),
        Some("py") | Some("js") | Some("jsx") | Some("ts") | Some("tsx") | Some("rs") | Some("go") | Some("c") | Some("cpp") | Some("h") | Some("hpp") | Some("java") | Some("kt") | Some("swift") | Some("rb") | Some("sh") | Some("ps1") | Some("toml") | Some("yaml") | Some("yml") | Some("xml") | Some("ini") | Some("cfg") | Some("conf") => Some(extract_code),
        Some("ipynb") => Some(extract_ipynb),
        Some("pdf") => Some(extract_pdf),
        Some("docx") => Some(extract_docx),
        Some("xlsx") | Some("xls") | Some("ods") => Some(extract_xlsx),
        _ => None,
    }
}

pub fn extract(path: &Path) -> AppResult<Extraction> {
    if let Some(f) = extractor_for(path) {
        return f(path);
    }
    // Last resort: read as UTF-8 and return the text.
    let text = std::fs::read_to_string(path).map_err(AppError::Io)?;
    Ok(Extraction { text, mime_type: "text/plain".into(), metadata: serde_json::json!({}) })
}

fn extract_text(path: &Path) -> AppResult<Extraction> {
    // Tolerate non-UTF-8 text files (cp1252, etc.) so a Markdown
    // file saved in a Windows codepage is still indexed rather
    // than recorded as `failed`. See `extract_code` for the
    // rationale.
    let bytes = std::fs::read(path).map_err(AppError::Io)?;
    let (text, non_utf8_replaced) = match std::str::from_utf8(&bytes) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (String::from_utf8_lossy(&bytes).into_owned(), true),
    };
    Ok(Extraction {
        text,
        mime_type: "text/plain".into(),
        metadata: serde_json::json!({ "non_utf8_replaced": non_utf8_replaced }),
    })
}

fn extract_json(path: &Path) -> AppResult<Extraction> {
    let raw = std::fs::read_to_string(path).map_err(AppError::Io)?;
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    let text = flatten_json(&value, "", 0);
    Ok(Extraction { text, mime_type: "application/json".into(), metadata: serde_json::json!({"valid": !value.is_null()}) })
}

fn flatten_json(v: &serde_json::Value, prefix: &str, depth: usize) -> String {
    if depth > 8 { return String::new(); }
    match v {
        serde_json::Value::String(s) => format!("{prefix}{s}\n"),
        serde_json::Value::Number(n) => format!("{prefix}{n}\n"),
        serde_json::Value::Bool(b) => format!("{prefix}{b}\n"),
        serde_json::Value::Null => format!("{prefix}null\n"),
        serde_json::Value::Array(arr) => arr.iter().map(|x| flatten_json(x, prefix, depth + 1)).collect(),
        serde_json::Value::Object(obj) => obj.iter()
            .map(|(k, v)| {
                let next = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten_json(v, &next, depth + 1)
            })
            .collect(),
    }
}

fn extract_csv(path: &Path) -> AppResult<Extraction> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .map_err(|e| AppError::Upstream(format!("csv: {e}")))?;
    let mut out = String::new();
    if let Some(headers) = rdr.headers().ok().map(|h| h.clone()) {
        out.push_str(&headers.iter().collect::<Vec<_>>().join("\t"));
        out.push('\n');
    }
    let mut rows = 0usize;
    for rec in rdr.records() {
        if rows > 50_000 { break; } // safety cap
        let Ok(r) = rec else { continue; };
        out.push_str(&r.iter().collect::<Vec<_>>().join("\t"));
        out.push('\n');
        rows += 1;
    }
    Ok(Extraction { text: out, mime_type: "text/csv".into(), metadata: serde_json::json!({"rows": rows}) })
}

fn extract_html(path: &Path) -> AppResult<Extraction> {
    let raw = std::fs::read_to_string(path).map_err(AppError::Io)?;
    let doc = scraper::Html::parse_document(&raw);
    // Concatenate all paragraph-like elements.
    let mut out = String::new();
    for sel in ["p", "h1", "h2", "h3", "li", "pre", "code"] {
        for el in doc.select(&scraper::Selector::parse(sel).unwrap()) {
            let t: String = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
            if !t.is_empty() { out.push_str(&t); out.push('\n'); }
        }
    }
    let title = doc.select(&scraper::Selector::parse("title").unwrap()).next()
        .map(|e| e.text().collect::<String>())
        .unwrap_or_default();
    Ok(Extraction { text: out, mime_type: "text/html".into(), metadata: serde_json::json!({"title": title}) })
}

fn extract_code(path: &Path) -> AppResult<Extraction> {
    // Source files are sometimes saved in a Windows codepage
    // (cp1252 / cp1251) instead of UTF-8. `read_to_string` would
    // fail outright on a non-UTF-8 byte and would record the
    // document as `failed`, which is the wrong outcome: the file is
    // perfectly readable, just not in UTF-8. Read the bytes and
    // decode them with `String::from_utf8_lossy`, which replaces
    // invalid sequences with the Unicode replacement character so
    // the rest of the file remains intact. The line-number prefix
    // still works because the replacement character is a valid
    // Unicode scalar.
    let bytes = std::fs::read(path).map_err(AppError::Io)?;
    let (raw, non_utf8_replaced) = match std::str::from_utf8(&bytes) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (String::from_utf8_lossy(&bytes).into_owned(), true),
    };
    // Preserve line numbers so the assistant can refer to them.
    let mut out = String::new();
    for (i, line) in raw.lines().enumerate() {
        out.push_str(&format!("{:>5}  {}\n", i + 1, line));
    }
    Ok(Extraction {
        text: out,
        mime_type: "text/x-code".into(),
        metadata: serde_json::json!({
            "line_count": raw.lines().count(),
            "non_utf8_replaced": non_utf8_replaced,
        }),
    })
}

fn extract_ipynb(path: &Path) -> AppResult<Extraction> {
    let raw = std::fs::read_to_string(path).map_err(AppError::Io)?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| AppError::Upstream(format!("ipynb json: {e}")))?;
    let mut out = String::new();
    let mut cell_count = 0usize;
    let mut code_cells = 0usize;
    let mut md_cells = 0usize;
    if let Some(cells) = v.get("cells").and_then(|c| c.as_array()) {
        for (i, cell) in cells.iter().enumerate() {
            let kind = cell.get("cell_type").and_then(|x| x.as_str()).unwrap_or("code");
            cell_count += 1;
            out.push_str(&format!("--- cell {i} ({kind}) ---\n"));
            let src = cell.get("source").map(|s| match s {
                serde_json::Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(""),
                serde_json::Value::String(s) => s.clone(),
                _ => String::new(),
            }).unwrap_or_default();
            if kind == "markdown" { md_cells += 1; }
            if kind == "code" { code_cells += 1; }
            out.push_str(&src);
            out.push('\n');
            if let Some(outputs) = cell.get("outputs").and_then(|o| o.as_array()) {
                for out_cell in outputs {
                    let text = match out_cell.get("text") {
                        Some(serde_json::Value::Array(arr)) => arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(""),
                        Some(serde_json::Value::String(s)) => s.clone(),
                        _ => out_cell.get("data").and_then(|d| d.get("text/plain")).and_then(|t| t.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(""))
                            .unwrap_or_default(),
                    };
                    if !text.is_empty() { out.push_str(&format!("[output] {text}\n")); }
                }
            }
        }
    }
    let lang = v.get("metadata").and_then(|m| m.get("kernelspec")).and_then(|k| k.get("language")).and_then(|x| x.as_str()).unwrap_or("python").to_string();
    Ok(Extraction {
        text: out,
        mime_type: "application/x-ipynb".into(),
        metadata: serde_json::json!({"cells": cell_count, "code": code_cells, "md": md_cells, "language": lang}),
    })
}

fn extract_pdf(path: &Path) -> AppResult<Extraction> {
    let bytes = std::fs::read(path).map_err(AppError::Io)?;
    let out = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| AppError::Upstream(format!("pdf: {e}")))?;
    Ok(Extraction { text: out, mime_type: "application/pdf".into(), metadata: serde_json::json!({}) })
}

fn extract_docx(path: &Path) -> AppResult<Extraction> {
    let file = std::fs::File::open(path).map_err(AppError::Io)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| AppError::Upstream(format!("docx zip: {e}")))?;
    let mut doc = docx_rs::Docx::default();
    {
        let mut f = zip.by_name("word/document.xml").map_err(|e| AppError::Upstream(format!("docx document.xml: {e}")))?;
        let mut s = String::new();
        std::io::Read::read_to_string(&mut f, &mut s).map_err(AppError::Io)?;
        // docx-rs parses the XML; we just want the text. Use a simple XML walk.
        let parsed = scraper::Html::parse_document(&format!("<root xmlns='x'>{s}</root>"));
        for p in parsed.select(&scraper::Selector::parse("p, h1, h2, h3, li").unwrap()) {
            let t: String = p.text().collect::<Vec<_>>().join(" ").trim().to_string();
            if !t.is_empty() { doc = doc.add_paragraph(docx_rs::Paragraph::default().add_run(docx_rs::Run::default().add_text(t.clone()))); }
        }
        let _ = doc; // silence
    }
    // Simpler: just walk the document.xml and concatenate text nodes.
    let mut f = zip.by_name("word/document.xml").map_err(|e| AppError::Upstream(format!("docx document.xml: {e}")))?;
    let mut s = String::new();
    std::io::Read::read_to_string(&mut f, &mut s).map_err(AppError::Io)?;
    // Extract text between <w:t...>...</w:t> tags.
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(open) = rest.find("<w:t") {
        rest = &rest[open..];
        if let Some(close_tag) = rest.find('>') {
            rest = &rest[close_tag + 1..];
            if let Some(end) = rest.find("</w:t>") {
                out.push_str(&rest[..end]);
                out.push('\n');
                rest = &rest[end + 6..];
            } else { break; }
        } else { break; }
    }
    Ok(Extraction { text: out, mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(), metadata: serde_json::json!({}) })
}

fn extract_xlsx(path: &Path) -> AppResult<Extraction> {
    use calamine::Reader;
    let mut out = String::new();
    let mut sheets_meta = serde_json::Map::new();
    let mut workbook = calamine::open_workbook_auto(path).map_err(|e| AppError::Upstream(format!("xlsx: {e}")))?;
    let names: Vec<String> = workbook.sheet_names().to_vec();
    for name in &names {
        let range = workbook.worksheet_range(name).map_err(|e| AppError::Upstream(format!("xlsx sheet {name}: {e}")))?;
        out.push_str(&format!("# sheet: {name}\n"));
        let (h, w) = range.get_size();
        sheets_meta.insert(name.clone(), serde_json::json!({"rows": h, "cols": w}));
        let max_rows = h.min(5_000);
        for r in 0..max_rows {
            let mut row_strs: Vec<String> = Vec::with_capacity(w as usize);
            for c in 0..w {
                let v = range.get_value((r as u32, c as u32)).map(|x| x.to_string()).unwrap_or_default();
                row_strs.push(v);
            }
            out.push_str(&row_strs.join("\t"));
            out.push('\n');
        }
    }
    Ok(Extraction {
        text: out,
        mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
        metadata: serde_json::json!({"sheets": sheets_meta}),
    })
}