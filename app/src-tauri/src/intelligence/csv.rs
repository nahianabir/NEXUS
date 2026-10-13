//! CSV / TSV export. We accept a `Table` (from `tables::extract`)
//! and emit RFC 4180 CSV by default. The user can opt into TSV by
//! setting the delimiter to '\t'. Quoting follows the spec:
//!   * wrap a cell in double quotes if it contains the delimiter,
//!     a quote, or a newline.
//!   * escape embedded quotes by doubling them.

use super::tables::Table;

pub struct CsvOptions {
    pub delimiter: char,
    pub include_header: bool,
}

impl Default for CsvOptions {
    fn default() -> Self { Self { delimiter: ',', include_header: true } }
}

pub fn to_csv(t: &Table, opts: &CsvOptions) -> String {
    let mut out = String::new();
    if opts.include_header {
        write_row(&mut out, &t.headers, opts.delimiter);
    }
    for row in &t.rows {
        write_row(&mut out, row, opts.delimiter);
    }
    out
}

fn write_row(out: &mut String, row: &[String], delim: char) {
    for (i, cell) in row.iter().enumerate() {
        if i > 0 { out.push(delim); }
        if needs_quote(cell, delim) {
            out.push('"');
            for c in cell.chars() {
                if c == '"' { out.push('"'); out.push('"'); }
                else { out.push(c); }
            }
            out.push('"');
        } else {
            out.push_str(cell);
        }
    }
    out.push_str("\r\n");
}

fn needs_quote(s: &str, delim: char) -> bool {
    s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_csv() {
        let t = Table { source_offset: 0, headers: vec!["a".into(), "b".into()], rows: vec![vec!["1".into(), "2".into()]] };
        let csv = to_csv(&t, &CsvOptions::default());
        assert_eq!(csv, "a,b\r\n1,2\r\n");
    }

    #[test]
    fn quotes_and_commas() {
        let t = Table { source_offset: 0, headers: vec!["name".into(), "note".into()], rows: vec![vec!["a, b".into(), "she said \"hi\"".into()]] };
        let csv = to_csv(&t, &CsvOptions::default());
        assert!(csv.contains("\"a, b\""));
        assert!(csv.contains("\"she said \"\"hi\"\"\""));
    }

    #[test]
    fn tsv_uses_tab() {
        let t = Table { source_offset: 0, headers: vec!["a".into(), "b".into()], rows: vec![vec!["x".into(), "y".into()]] };
        let tsv = to_csv(&t, &CsvOptions { delimiter: '\t', include_header: true });
        assert!(tsv.contains('\t'));
        assert!(!tsv.contains(','));
    }
}