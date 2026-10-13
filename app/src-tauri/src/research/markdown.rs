//! Markdown report export for a research project.
//!
//! The report is a deterministic serialisation of the project's
//! papers, claims, matrices, and existing reports. It is intentionally
//! simple: no templates, no theme system. The point is that the user
//! can copy the output into a notes app, paste it into a paper, or
//! diff it across runs.

use super::{ComparisonMatrix, MatrixCell, MatrixColumn, PaperClaim, ResearchPaper, ResearchProject, ResearchReport};
use crate::db::SqlitePool;
use crate::error::AppResult;

/// Bundle everything in one struct so callers don't have to fetch
/// each list separately.
pub struct ReportData {
    pub project: ResearchProject,
    pub papers: Vec<ResearchPaper>,
    pub claims: Vec<PaperClaim>,
    pub matrices: Vec<ComparisonMatrix>,
    pub cells_by_matrix: Vec<(ComparisonMatrix, Vec<MatrixColumn>, Vec<MatrixCell>)>,
    pub reports: Vec<ResearchReport>,
}

pub fn build_report(project: &ResearchProject, papers: &[ResearchPaper], claims: &[PaperClaim], matrices: &[(ComparisonMatrix, Vec<MatrixColumn>, Vec<MatrixCell>)], reports: &[ResearchReport]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", project.title));
    out.push_str(&format!("**Question.** {}\n\n", project.question));
    if let Some(desc) = &project.description {
        if !desc.trim().is_empty() { out.push_str(&format!("{desc}\n\n")); }
    }
    out.push_str("---\n\n");

    out.push_str("## Papers\n\n");
    if papers.is_empty() {
        out.push_str("_No papers added yet._\n\n");
    } else {
        for p in papers {
            out.push_str(&format!("### {}\n", p.title));
            let mut meta = Vec::new();
            if let Some(a) = &p.authors { if !a.is_empty() { meta.push(a.clone()); } }
            if let Some(v) = &p.venue { meta.push(v.clone()); }
            if let Some(y) = p.year { meta.push(y.to_string()); }
            if let Some(d) = &p.doi { meta.push(format!("DOI: {d}")); }
            if let Some(u) = &p.url { meta.push(u.clone()); }
            if !meta.is_empty() { out.push_str(&format!("_{}_\n\n", meta.join(" · "))); }
            if let Some(n) = &p.notes { if !n.is_empty() { out.push_str(&format!("{n}\n\n")); } }
        }
    }

    out.push_str("## Claims\n\n");
    if claims.is_empty() {
        out.push_str("_No claims extracted yet._\n\n");
    } else {
        let mut by_cat: std::collections::BTreeMap<String, Vec<&PaperClaim>> = std::collections::BTreeMap::new();
        for c in claims { by_cat.entry(c.category.clone()).or_default().push(c); }
        for (cat, items) in by_cat {
            out.push_str(&format!("### {}\n", capitalise(&cat)));
            for c in items {
                out.push_str(&format!("- {}\n", c.claim));
                if let Some(ev) = &c.evidence { if !ev.is_empty() { out.push_str(&format!("  - evidence: {ev}\n")); } }
                if let Some(pid) = &c.paper_id {
                    if let Some(p) = papers.iter().find(|p| &p.id == pid) {
                        out.push_str(&format!("  - paper: {}\n", p.title));
                    }
                }
            }
            out.push('\n');
        }
    }

    if !matrices.is_empty() {
        out.push_str("## Comparison matrices\n\n");
        for (m, cols, cells) in matrices {
            out.push_str(&format!("### {}\n", m.title));
            if let Some(desc) = &m.description { if !desc.is_empty() { out.push_str(&format!("{desc}\n")); } }
            // Markdown table: first column = paper title, then one column per criterion.
            if papers.is_empty() || cols.is_empty() {
                out.push_str("_Add at least one paper and one column to render the matrix._\n\n");
                continue;
            }
            // Header.
            out.push_str("| Paper |");
            for col in cols { out.push_str(&format!(" {} |", col.label)); }
            out.push('\n');
            // Separator.
            out.push_str("|---|");
            for _ in cols { out.push_str("---|"); }
            out.push('\n');
            for p in papers {
                out.push_str(&format!("| {} |", escape_pipe(&p.title)));
                for col in cols {
                    let cell = cells.iter().find(|c| c.paper_id == p.id && c.column_id == col.id);
                    let rendered = match cell {
                        Some(c) => render_cell(c, &col.kind),
                        None => String::new(),
                    };
                    out.push_str(&format!(" {} |", escape_pipe(&rendered)));
                }
                out.push('\n');
            }
            out.push('\n');
        }
    }

    if !reports.is_empty() {
        out.push_str("## Saved reports\n\n");
        for r in reports {
            out.push_str(&format!("- **{}** (updated {})\n", r.title, format_time(r.updated_at)));
        }
        out.push('\n');
    }

    out
}

fn render_cell(c: &MatrixCell, kind: &str) -> String {
    match kind {
        "number" => c.value_number.map(|v| format!("{v}")).unwrap_or_default(),
        "bool" => c.value_bool.map(|v| if v { "yes" } else { "no" }.to_string()).unwrap_or_default(),
        "rating" => c.value_number.map(|v| render_stars(v)).unwrap_or_default(),
        _ => c.value_text.clone().unwrap_or_default(),
    }
}

fn render_stars(v: f64) -> String {
    // 0..5 scale.
    let n = (v.clamp(0.0, 5.0)).round() as i64;
    let filled: String = std::iter::repeat('★').take(n as usize).collect();
    let empty: String = std::iter::repeat('☆').take(5 - n as usize).collect();
    format!("{filled}{empty}")
}

fn escape_pipe(s: &str) -> String { s.replace('|', "\\|").replace('\n', " ") }

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn format_time(ts: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts)
        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| ts.to_string())
}

/// Fetch everything for a project and produce the Markdown body.
pub fn build_project_report(pool: &SqlitePool, project_id: &str) -> AppResult<String> {
    let project = super::projects::get(pool, project_id)?;
    let papers = super::papers::list_for(pool, project_id)?;
    let claims = super::claims::list_for(pool, project_id, None, None)?;
    let matrices = super::matrices::list_matrices(pool, project_id)?;
    let mut bundle: Vec<(ComparisonMatrix, Vec<MatrixColumn>, Vec<MatrixCell>)> = Vec::new();
    for m in matrices {
        let cols = super::matrices::list_columns(pool, &m.id)?;
        let cells = super::matrices::list_cells(pool, &m.id)?;
        bundle.push((m, cols, cells));
    }
    let reports = super::reports::list_for(pool, project_id)?;
    Ok(build_report(&project, &papers, &claims, &bundle, &reports))
}
