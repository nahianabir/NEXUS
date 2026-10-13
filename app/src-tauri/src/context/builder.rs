//! The builder.

use std::collections::BTreeMap;

use crate::db::SqlitePool;
use crate::error::AppResult;
use crate::ingestion;
use crate::kg;
use crate::mempalace::state::MemoryBackendState;
use crate::memory_v2;
use crate::projects;
use super::types::*;

/// Inputs to the builder. The chat command fills these in based on
/// the active project and the user message.
#[derive(Debug, Clone, Default)]
pub struct BuildRequest {
    pub workspace_id: Option<String>,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub user_message: String,
    /// Approximate token budget for the entire `system_block`. The
    /// builder drops lowest-score items first when this is exceeded.
    pub max_tokens: i64,
    /// Optional explicit memories to always include (e.g. pinned facts).
    pub pinned: Vec<memory_v2::memory::MemoryV2>,
}

/// Heuristic: count tokens as roughly `chars/4` (English). We deliberately
/// avoid pulling in a real tokenizer: the budget is an upper bound, not a
/// pricing engine. The frontend can show a more accurate count if needed.
pub fn approx_tokens(s: &str) -> i64 {
    ((s.chars().count() as f64) / 4.0).ceil() as i64
}

pub fn build(
    pool: &SqlitePool,
    backends: &MemoryBackendState,
    req: &BuildRequest,
) -> AppResult<ContextBundle> {
    let mut items: Vec<ContextItem> = Vec::new();
    let mut sources_searched: Vec<ContextSource> = Vec::new();

    // 1. Project context.
    if let Some(pid) = &req.project_id {
        if let Ok(project) = projects::projects::get(pool, pid) {
            sources_searched.push(ContextSource::ProjectContext);
            let mut block = String::new();
            if !project.goals.is_empty() { block.push_str(&format!("Goals: {}\n", project.goals.join("; "))); }
            if let Some(inst) = &project.instructions { block.push_str(&format!("Project instructions: {inst}\n")); }
            if let Some(desc) = &project.description { block.push_str(&format!("Description: {desc}\n")); }
            if !block.is_empty() {
                items.push(ContextItem {
                    source: ContextSource::ProjectContext,
                    source_id: project.id.clone(),
                    source_label: Some(project.name.clone()),
                    title: Some("active project".into()),
                    excerpt: block.trim().to_string(),
                    approx_tokens: approx_tokens(&block),
                    provenance: Some("projects".into()),
                    verification: Some("user_stated".into()),
                    score: 100.0,
                });
            }
            // Project notes contribute too.
            if let Ok(notes) = projects::notes::list(pool, Some(pid)) {
                for n in notes.iter().take(5) {
                    items.push(ContextItem {
                        source: ContextSource::ProjectContext,
                        source_id: n.id.clone(),
                        source_label: Some(n.title.clone()),
                        title: Some("project note".into()),
                        excerpt: if n.body.len() > 400 { format!("{}…", &n.body[..400]) } else { n.body.clone() },
                        approx_tokens: approx_tokens(&n.body),
                        provenance: Some(format!("notes:{}", n.id)),
                        verification: Some("user_stated".into()),
                        score: 80.0,
                    });
                }
            }
        }
    }

    // 2. Pinned memories first.
    for m in &req.pinned {
        items.push(ContextItem {
            source: ContextSource::NativeMemory,
            source_id: m.id.clone(),
            source_label: m.source_label.clone(),
            title: Some(m.category.clone()),
            excerpt: m.content.clone(),
            approx_tokens: approx_tokens(&m.content),
            provenance: Some(format!("memories_v2:{}", m.id)),
            verification: Some(m.verification.clone()),
            score: 95.0,
        });
    }
    sources_searched.push(ContextSource::NativeMemory);

    // 3. Native memory retrieval (FTS5-based search).
    if !req.user_message.trim().is_empty() {
        if let Ok(ids) = memory_v2::search::search(pool, &req.user_message, None, req.project_id.as_deref(), 25) {
            for id in ids {
                if let Ok(m) = memory_v2::memory::get(pool, &id) {
                    // Already pinned? Skip the duplicate.
                    if req.pinned.iter().any(|p| p.id == m.id) { continue; }
                    let score = compute_memory_score(&m, &req.user_message);
                    items.push(ContextItem {
                        source: ContextSource::NativeMemory,
                        source_id: m.id.clone(),
                        source_label: m.source_label.clone(),
                        title: Some(m.category.clone()),
                        excerpt: m.content.clone(),
                        approx_tokens: approx_tokens(&m.content),
                        provenance: Some(format!("memories_v2:{}", m.id)),
                        verification: Some(m.verification.clone()),
                        score,
                    });
                }
            }
        }
    }

    // 4. MemPalace (best-effort, falls back silently).
    if !req.user_message.trim().is_empty() {
        sources_searched.push(ContextSource::MemPalaceMemory);
        let results = backends.search(&req.user_message, req.project_id.as_deref(), 10);
        for r in results {
            items.push(ContextItem {
                source: ContextSource::MemPalaceMemory,
                source_id: r.id.clone(),
                source_label: None,
                title: Some(r.category.clone()),
                excerpt: r.content.clone(),
                approx_tokens: approx_tokens(&r.content),
                provenance: Some(format!("mempalace:{}", r.id)),
                verification: None,
                score: 70.0,
            });
        }
    }

    // 5. Knowledge Vault (FTS5 + vector if available).
    if !req.user_message.trim().is_empty() {
        sources_searched.push(ContextSource::VaultDocument);
        if let Ok(found) = ingestion::search::search_fts(pool, &req.user_message, 20) {
            for (chunk_id, s) in found {
                if let Ok(Some(chunk)) = ingestion::pipeline::get_chunk(pool, &chunk_id) {
                    items.push(ContextItem {
                        source: ContextSource::VaultDocument,
                        source_id: chunk.id.clone(),
                        source_label: Some(chunk.document_path.clone()),
                        title: None,
                        excerpt: chunk.text.clone(),
                        approx_tokens: approx_tokens(&chunk.text),
                        provenance: Some(format!("chunks:{}:{}", chunk.document_id, chunk.ordinal)),
                        verification: Some("indexed".into()),
                        score: s + 30.0,
                    });
                }
            }
        }
    }

    // 6. Knowledge graph. The graph only contributes if we can match
    // the user message to a known entity by name. We do not invent
    // nodes; if nothing matches, this section is empty.
    if !req.user_message.trim().is_empty() {
        sources_searched.push(ContextSource::GraphEntity);
        if let Ok(candidates) = kg::queries::search(pool, &req.user_message, 5) {
            for c in candidates.into_iter().take(3) {
                items.push(ContextItem {
                    source: ContextSource::GraphEntity,
                    source_id: c.id.clone(),
                    source_label: None,
                    title: Some(format!("{} entity", c.entity_type)),
                    excerpt: format!("{} ({})", c.canonical_name,
                        c.description.clone().unwrap_or_else(|| "no description".into())),
                    approx_tokens: approx_tokens(&c.canonical_name),
                    provenance: Some(format!("kg_entities:{}", c.id)),
                    verification: Some("indexed".into()),
                    score: 60.0,
                });
                if let Ok(hood) = kg::queries::neighborhood(pool, &c.id, kg::Bounded { max_depth: 1, max_nodes: 16 }) {
                    for (i, edge) in hood.edges.iter().enumerate() {
                        let src = hood.nodes.iter().find(|n| n.id == edge.source_entity_id).map(|n| n.canonical_name.clone()).unwrap_or_default();
                        let tgt = hood.nodes.iter().find(|n| n.id == edge.target_entity_id).map(|n| n.canonical_name.clone()).unwrap_or_default();
                        let excerpt = format!("{src} --{}--> {tgt}", edge.relationship_type);
                        let evidence = hood.evidence_by_rel.get(i).map(|e| e.len()).unwrap_or(0);
                        items.push(ContextItem {
                            source: ContextSource::GraphRelationship,
                            source_id: edge.id.clone(),
                            source_label: None,
                            title: Some(format!("edge · {}", edge.relationship_type)),
                            excerpt,
                            approx_tokens: 16,
                            provenance: Some(format!("kg_relationships:{}", edge.id)),
                            verification: Some(edge.verification_status.clone()),
                            score: 50.0,
                            // Embed evidence count in the provenance suffix
                        }.with_provenance_extra(format!("evidence:{evidence}")));
                    }
                }
            }
        }
    }

    // Sort by score desc, then drop the lowest-score items until we
    // are under the budget. Pinned items (score 95) survive by virtue
    // of being on top; explicit pinned memories are added with score
    // 100 above so they are always first.
    items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut total = 0i64;
    let mut kept: Vec<ContextItem> = Vec::new();
    let mut truncated = false;
    for item in items {
        if total + item.approx_tokens > req.max_tokens.max(64) {
            truncated = true;
            continue;
        }
        total += item.approx_tokens;
        kept.push(item);
    }

    // Build the system block.
    let mut system = String::new();
    system.push_str("# NEXUS Context\n\n");
    let mut by_source: BTreeMap<String, Vec<&ContextItem>> = BTreeMap::new();
    for it in &kept { by_source.entry(it.source.label().to_string()).or_default().push(it); }
    for (label, items) in &by_source {
        system.push_str(&format!("## {}\n", label));
        for (i, it) in items.iter().enumerate() {
            let header = it.title.clone().unwrap_or_else(|| label.clone());
            system.push_str(&format!("{i}. **{header}**"));
            if let Some(s) = &it.source_label { system.push_str(&format!(" ({s})")); }
            system.push('\n');
            system.push_str(&format!("   {}\n", it.excerpt));
            if let Some(p) = &it.provenance { system.push_str(&format!("   _source: {p}_\n")); }
            system.push('\n');
        }
    }

    // Per-source counts.
    let mut hits_by_source: BTreeMap<String, i64> = BTreeMap::new();
    for it in &kept { *hits_by_source.entry(it.source.label().to_string()).or_default() += 1; }

    Ok(ContextBundle {
        items: kept,
        approx_tokens: total,
        truncated,
        system_block: system,
        sources_searched,
        hits_by_source,
    })
}

fn compute_memory_score(m: &memory_v2::memory::MemoryV2, query: &str) -> f64 {
    let q = query.to_ascii_lowercase();
    let content_lc = m.content.to_ascii_lowercase();
    let needle_count = q.split_whitespace().filter(|w| !w.is_empty() && content_lc.contains(w)).count() as f64;
    let denom = (q.split_whitespace().count() as f64).max(1.0);
    let base = (needle_count / denom) * 60.0;
    let verified_bonus = if m.verified_at.is_some() { 10.0 } else { 0.0 };
    let pinned_bonus = if m.pinned { 5.0 } else { 0.0 };
    let type_bonus = match m.category.as_str() {
        "instruction" | "preference" | "decision" => 10.0,
        "constraint" | "problem" => 8.0,
        _ => 0.0,
    };
    base + verified_bonus + pinned_bonus + type_bonus
}

trait WithProvenanceExtra {
    fn with_provenance_extra(self, extra: String) -> Self;
}

impl WithProvenanceExtra for ContextItem {
    fn with_provenance_extra(mut self, extra: String) -> Self {
        self.provenance = Some(match self.provenance {
            Some(p) => format!("{p};{extra}"),
            None => extra,
        });
        self
    }
}
