//! Table extraction. Pure Rust, no external dependency.
//!
//! Phase J supports the two formats users actually paste into
//! notes and chat:
//!   1. Pipe tables (Markdown):
//!      | col1 | col2 |
//!      | ---- | ---- |
//!      | a    | b    |
//!   2. Whitespace-aligned tables (tab or 2+ spaces):
//!      col1   col2
//!      ----   ----
//!      a      b
//!
//! The heuristic for whitespace tables: a row with 2+ non-empty
//! cells separated by runs of 2+ whitespace. The first non-empty
//! row with at least 2 cells is treated as the header. Anything
//! that looks like a separator (e.g. "----  ----") is consumed but
//! not emitted.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub source_offset: usize,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn extract(text: &str) -> Vec<Table> {
    let mut out = Vec::new();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        // Try pipe first.
        if let Some(t) = try_pipe(&lines, i) {
            out.push(t.0);
            i = t.1;
            continue;
        }
        // Then whitespace.
        if let Some(t) = try_whitespace(&lines, i) {
            out.push(t.0);
            i = t.1;
            continue;
        }
        i += 1;
    }
    out
}

fn try_pipe(lines: &[&str], start: usize) -> Option<(Table, usize)> {
    let first = lines.get(start)?.trim();
    if !first.contains('|') { return None; }
    let cells = split_pipe(first);
    if cells.is_empty() { return None; }
    // We need at least 2 rows total: header + body.
    if start + 1 >= lines.len() { return None; }
    let second = lines[start + 1].trim();
    // The separator must have at least one "---" and as many "|" as cells.
    if !is_pipe_separator(second) { return None; }
    let mut rows = Vec::new();
    let mut j = start + 2;
    while j < lines.len() {
        let line = lines[j].trim();
        if line.is_empty() { break; }
        if !line.contains('|') { break; }
        let row = split_pipe(line);
        rows.push(row);
        j += 1;
    }
    Some((Table { source_offset: start, headers: cells, rows }, j))
}

fn is_pipe_separator(s: &str) -> bool {
    let parts: Vec<&str> = s.split('|').map(str::trim).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() { return false; }
    parts.iter().all(|p| p.chars().all(|c| c == '-' || c == ':') && p.len() >= 3)
}

fn split_pipe(s: &str) -> Vec<String> {
    s.trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn try_whitespace(lines: &[&str], start: usize) -> Option<(Table, usize)> {
    let first = lines.get(start)?;
    let cells = split_whitespace(first);
    if cells.len() < 2 { return None; }
    // The next line must look like a separator with at least 2 cells.
    if start + 1 >= lines.len() { return None; }
    let second = lines[start + 1];
    let sep_cells = split_whitespace(second);
    if sep_cells.len() != cells.len() { return None; }
    if !sep_cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == '_' || ch == '=') && c.len() >= 2) {
        return None;
    }
    let mut rows = Vec::new();
    let mut j = start + 2;
    while j < lines.len() {
        let line = lines[j];
        if line.trim().is_empty() { break; }
        let row = split_whitespace(line);
        if row.len() != cells.len() { break; }
        rows.push(row);
        j += 1;
    }
    Some((Table { source_offset: start, headers: cells, rows }, j))
}

fn split_whitespace(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut in_run = false;
    let mut spaces = 0;
    for c in s.chars() {
        if c.is_whitespace() {
            if in_run { spaces += 1; } else { spaces = 1; in_run = true; }
            if spaces >= 2 && !buf.is_empty() {
                out.push(std::mem::take(&mut buf));
                in_run = false;
                spaces = 0;
            }
        } else {
            if in_run { spaces = 0; in_run = false; }
            buf.push(c);
        }
    }
    if !buf.is_empty() { out.push(buf); }
    out
}

/// Render a table back out as Markdown pipe-table text. Useful when
/// the user wants to copy a normalized version of an extracted table.
pub fn render_markdown(t: &Table) -> String {
    let widths: Vec<usize> = {
        let mut w: Vec<usize> = t.headers.iter().map(|h| h.chars().count()).collect();
        for row in &t.rows {
            for (i, cell) in row.iter().enumerate() {
                if i >= w.len() { w.push(0); }
                w[i] = w[i].max(cell.chars().count());
            }
        }
        w
    };
    let mut out = String::new();
    out.push('|');
    for (i, h) in t.headers.iter().enumerate() {
        let pad = widths.get(i).copied().unwrap_or(0);
        out.push_str(&format!(" {} |", pad_with(&pad_text(h, pad), pad)));
    }
    out.push('\n');
    out.push('|');
    for w in &widths {
        out.push_str(&format!("{}|", "-".repeat(*w + 2)));
    }
    out.push('\n');
    for row in &t.rows {
        out.push('|');
        for (i, cell) in row.iter().enumerate() {
            let pad = widths.get(i).copied().unwrap_or(0);
            out.push_str(&format!(" {} |", pad_with(&pad_text(cell, pad), pad)));
        }
        out.push('\n');
    }
    out
}

fn pad_text(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width { s.to_string() } else { format!("{}{}", s, " ".repeat(width - n)) }
}

fn pad_with(s: &str, _width: usize) -> String { s.to_string() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_pipe_table() {
        let text = "intro\n| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n\nafter\n";
        let t = extract(text);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].headers, vec!["a", "b"]);
        assert_eq!(t[0].rows.len(), 2);
    }

    #[test]
    fn extract_whitespace_table() {
        let text = "before\nName  Age\n----  ---\nAda   36\nBob   24\nafter\n";
        let t = extract(text);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].headers, vec!["Name", "Age"]);
        assert_eq!(t[0].rows.len(), 2);
        assert_eq!(t[0].rows[0], vec!["Ada", "36"]);
    }

    #[test]
    fn render_round_trip() {
        let text = "| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        let t = extract(text);
        let out = render_markdown(&t[0]);
        assert!(out.starts_with("| a | b |"));
        assert!(out.contains("| 1 | 2 |"));
    }

    #[test]
    fn no_table_returns_empty() {
        let t = extract("just a single line\nwith no tables\n");
        assert!(t.is_empty());
    }
}