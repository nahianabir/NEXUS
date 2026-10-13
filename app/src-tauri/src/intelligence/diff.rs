//! Document comparison. Produces a small, line-level diff summary
//! suitable for the "compare two versions" use case. We do not
//! implement Myers diff; we use a simple longest-common-subsequence
//! approximation that's good enough for the documents the vault
//! ingests (markdown notes, plain text).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: String, // 'equal' | 'add' | 'remove'
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub lines: Vec<DiffLine>,
    pub added: usize,
    pub removed: usize,
    pub same: usize,
}

pub fn diff_lines(a: &str, b: &str) -> DiffSummary {
    let a_lines: Vec<&str> = a.split('\n').collect();
    let b_lines: Vec<&str> = b.split('\n').collect();
    // Compute LCS table.
    let n = a_lines.len();
    let m = b_lines.len();
    let mut table = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            table[i + 1][j + 1] = if a_lines[i] == b_lines[j] { table[i][j] + 1 } else { table[i + 1][j].max(table[i][j + 1]) };
        }
    }
    // Walk the table to produce the diff.
    let mut lines = Vec::new();
    let mut i = n;
    let mut j = m;
    let mut added = 0;
    let mut removed = 0;
    let mut same = 0;
    while i > 0 && j > 0 {
        if a_lines[i - 1] == b_lines[j - 1] {
            lines.push(DiffLine { kind: "equal".into(), text: a_lines[i - 1].to_string() });
            same += 1;
            i -= 1; j -= 1;
        } else if table[i - 1][j] >= table[i][j - 1] {
            lines.push(DiffLine { kind: "remove".into(), text: a_lines[i - 1].to_string() });
            removed += 1;
            i -= 1;
        } else {
            lines.push(DiffLine { kind: "add".into(), text: b_lines[j - 1].to_string() });
            added += 1;
            j -= 1;
        }
    }
    while i > 0 {
        lines.push(DiffLine { kind: "remove".into(), text: a_lines[i - 1].to_string() });
        removed += 1; i -= 1;
    }
    while j > 0 {
        lines.push(DiffLine { kind: "add".into(), text: b_lines[j - 1].to_string() });
        added += 1; j -= 1;
    }
    lines.reverse();
    DiffSummary { lines, added, removed, same }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_inputs_have_no_changes() {
        let d = diff_lines("a\nb\nc", "a\nb\nc");
        assert_eq!(d.added, 0);
        assert_eq!(d.removed, 0);
        assert_eq!(d.same, 3);
    }

    #[test]
    fn detects_additions_and_removals() {
        let d = diff_lines("a\nb\nc", "a\nc\nd");
        assert_eq!(d.removed, 1);
        assert_eq!(d.added, 1);
        assert_eq!(d.same, 2);
    }

    #[test]
    fn diff_summary_is_walkable() {
        let d = diff_lines("hello\nworld", "hello\nnexus");
        assert_eq!(d.lines.iter().filter(|l| l.kind == "equal").count(), 1);
        assert!(d.lines.iter().any(|l| l.kind == "remove" && l.text == "world"));
        assert!(d.lines.iter().any(|l| l.kind == "add" && l.text == "nexus"));
    }
}