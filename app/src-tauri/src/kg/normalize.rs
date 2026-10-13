//! Normalisation for entity and alias names. Used by `find_or_create`
//! to avoid creating duplicate entities that differ only by case or
//! punctuation.
//!
//! We deliberately stay in ASCII: entity names in NEXUS are technical
//! identifiers (model names, dataset names, project names) and the
//! Unicode complications are not worth the extra dependency for this
//! product surface.

pub fn normalize_name(s: &str) -> String {
    let folded = s.to_ascii_lowercase();
    let mut out = String::with_capacity(folded.len());
    let mut prev_ws = false;
    for ch in folded.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_ws = false;
        } else if ch.is_whitespace() {
            if !prev_ws { out.push(' '); }
            prev_ws = true;
        }
        // skip punctuation and non-ASCII letters
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_folds_case_and_punctuation() {
        assert_eq!(normalize_name("Tauri  2!"), "tauri 2");
        assert_eq!(normalize_name("  Llama-3 "), "llama3");
    }
}
