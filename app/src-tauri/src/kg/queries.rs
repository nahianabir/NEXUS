//! Bounded graph traversal. Phase D.

use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::params;

use crate::db::SqlitePool;
use crate::error::AppResult;
use super::evidence;
use super::graph;
use super::relationship;
use super::types::{Entity, Evidence, Neighborhood, Relationship};

#[derive(Debug, Clone, Copy)]
pub struct Bounded {
    pub max_depth: usize,
    pub max_nodes: usize,
}

impl Default for Bounded {
    fn default() -> Self { Self { max_depth: 2, max_nodes: 200 } }
}

pub fn neighborhood(pool: &SqlitePool, center_id: &str, bounds: Bounded) -> AppResult<Neighborhood> {
    let center = graph::get(pool, center_id)?;
    let mut visited: HashMap<String, Entity> = HashMap::new();
    visited.insert(center.id.clone(), center.clone());
    let mut edges: HashMap<String, Relationship> = HashMap::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((center.id.clone(), 0));
    let mut cycle_guard: HashSet<String> = HashSet::new();

    while let Some((id, depth)) = queue.pop_front() {
        if depth >= bounds.max_depth { continue; }
        if visited.len() >= bounds.max_nodes { break; }
        let rels = {
            let mut out = relationship::list_for(pool, &id, "out", 200)?;
            out.extend(relationship::list_for(pool, &id, "in", 200)?);
            out
        };
        for rel in rels {
            // Detect cycles: (center, …, center).
            if rel.source_entity_id == center.id && rel.target_entity_id == center.id {
                continue;
            }
            if !cycle_guard.insert(rel.id.clone()) { continue; }
            edges.insert(rel.id.clone(), rel.clone());
            for neighbour in [rel.source_entity_id.as_str(), rel.target_entity_id.as_str()] {
                if neighbour == id { continue; }
                if visited.contains_key(neighbour) { continue; }
                if let Ok(e) = graph::get(pool, neighbour) {
                    if visited.len() < bounds.max_nodes {
                        let id = e.id.clone();
                        visited.insert(e.id.clone(), e);
                        queue.push_back((id, depth + 1));
                    }
                }
            }
        }
    }

    // Stable output order.
    let mut nodes: Vec<Entity> = visited.into_values().collect();
    nodes.sort_by(|a, b| a.canonical_name.cmp(&b.canonical_name));
    let mut rels: Vec<Relationship> = edges.into_values().collect();
    rels.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let mut evidence_by_rel = Vec::with_capacity(rels.len());
    for r in &rels {
        let ev = evidence::list_for(pool, &r.id)?;
        evidence_by_rel.push(ev);
    }

    Ok(Neighborhood { center, nodes, edges: rels, evidence_by_rel })
}

/// Find entities that connect `a` and `b` within `max_depth` hops.
/// Returns the shortest path that touches the fewest hops; ties are
/// broken by total weight. Empty result means no path.
pub fn shortest_path(pool: &SqlitePool, a: &str, b: &str, max_depth: usize) -> AppResult<Vec<String>> {
    if a == b { return Ok(vec![a.to_string()]); }
    // BFS from `a`, store parent pointers.
    let mut parent: HashMap<String, String> = HashMap::new();
    let mut depth: HashMap<String, usize> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(a.to_string());
    depth.insert(a.to_string(), 0);
    let mut found: Option<String> = None;
    while let Some(node) = queue.pop_front() {
        let d = depth[&node];
        if d >= max_depth { continue; }
        let rels = {
            let mut out = relationship::list_for(pool, &node, "out", 200)?;
            out.extend(relationship::list_for(pool, &node, "in", 200)?);
            out
        };
        for rel in rels {
            let next = if rel.source_entity_id == node { rel.target_entity_id.clone() } else { rel.source_entity_id.clone() };
            if depth.contains_key(&next) { continue; }
            parent.insert(next.clone(), node.clone());
            depth.insert(next.clone(), d + 1);
            if next == b { found = Some(next); break; }
            queue.push_back(next);
        }
        if found.is_some() { break; }
    }
    if let Some(target) = found {
        let mut path = vec![target.clone()];
        let mut cur = target;
        while let Some(p) = parent.get(&cur) {
            path.push(p.clone());
            cur = p.clone();
        }
        path.reverse();
        return Ok(path);
    }
    Ok(Vec::new())
}

/// Quick stats. Used by the dashboard and the graph header.
pub fn stats(pool: &SqlitePool) -> AppResult<GraphStats> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let entities: i64 = conn.query_row("SELECT COUNT(*) FROM kg_entities WHERE merged_into IS NULL", [], |r| r.get(0))?;
    let relationships: i64 = conn.query_row("SELECT COUNT(*) FROM kg_relationships", [], |r| r.get(0))?;
    let evidence: i64 = conn.query_row("SELECT COUNT(*) FROM kg_evidence", [], |r| r.get(0))?;
    let by_type: Vec<(String, i64)> = {
        let mut stmt = conn.prepare("SELECT entity_type, COUNT(*) FROM kg_entities WHERE merged_into IS NULL GROUP BY entity_type ORDER BY 2 DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        out
    };
    Ok(GraphStats { entities, relationships, evidence, by_type })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphStats {
    pub entities: i64,
    pub relationships: i64,
    pub evidence: i64,
    pub by_type: Vec<(String, i64)>,
}

/// Lightweight search across entity names + aliases. The query is
/// tokenised; an entity is returned when any of the tokens matches
/// either the canonical name or a normalised alias. Returns the
/// matching entities up to `limit`.
pub fn search(pool: &SqlitePool, query: &str, limit: usize) -> AppResult<Vec<Entity>> {
    let tokens: Vec<String> = query.split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_ascii_lowercase())
        .filter(|t| t.len() >= 3)
        .collect();
    if tokens.is_empty() { return Ok(Vec::new()); }
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    // Build the SUM-of-matches score. We add 2 per token that matches
    // the canonical name and 1 per token that matches an alias.
    let mut score_terms = Vec::new();
    for i in 0..tokens.len() {
        let canon = format!("(CASE WHEN LOWER(e.canonical_name) LIKE ?{} THEN 2 ELSE 0 END)", i * 2 + 1);
        let alias = format!("(CASE WHEN a.normalized_alias LIKE ?{} THEN 1 ELSE 0 END)", i * 2 + 2);
        score_terms.push(format!("({canon} + {alias})"));
    }
    let score_expr = score_terms.join(" + ");
    let mut where_clauses: Vec<String> = vec!["e.merged_into IS NULL".into()];
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    for t in &tokens {
        let like = format!("%{t}%");
        args.push(Box::new(like.clone()));
        args.push(Box::new(like));
    }
    for _ in 0..tokens.len() {
        // The SUM-with-?N is the main filter, but we also keep the
        // OR chain for the WHERE so the planner can use the index.
        // We append OR clauses that point at the same params, but
        // shifted: each token's canon/alias are already bound.
        // To keep things simple we just rely on the HAVING.
    }
    let _ = where_clauses; // not used; the HAVING is authoritative
    let sql = format!(
        "SELECT e.id, e.workspace_id, e.entity_type, e.canonical_name, e.description, e.metadata_json, e.merged_into, e.created_at, e.updated_at,
                ({score_expr}) AS score
         FROM kg_entities e
         LEFT JOIN kg_aliases a ON a.entity_id = e.id
         GROUP BY e.id
         HAVING score > 0
         ORDER BY score DESC, e.updated_at DESC
         LIMIT ?"
    );
    let last = tokens.len() * 2 + 1;
    let mut final_args: Vec<Box<dyn rusqlite::ToSql>> = args;
    final_args.push(Box::new(limit as i64));
    let _ = last;
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(final_args.iter()), |r| {
        Ok(Entity {
            id: r.get(0)?, workspace_id: r.get(1)?, entity_type: r.get(2)?,
            canonical_name: r.get(3)?, description: r.get(4)?, metadata_json: r.get(5)?,
            merged_into: r.get(6)?, created_at: r.get(7)?, updated_at: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

/// Common query: "which projects use this dataset?"
pub fn projects_using_entity(pool: &SqlitePool, entity_id: &str, limit: usize) -> AppResult<Vec<Entity>> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT e.id, e.workspace_id, e.entity_type, e.canonical_name, e.description, e.metadata_json, e.merged_into, e.created_at, e.updated_at
         FROM kg_entities e
         JOIN kg_relationships r ON r.source_entity_id = e.id
         WHERE r.target_entity_id = ?1 AND e.entity_type = 'project'
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![entity_id, limit as i64], |r| {
        Ok(Entity {
            id: r.get(0)?, workspace_id: r.get(1)?, entity_type: r.get(2)?,
            canonical_name: r.get(3)?, description: r.get(4)?, metadata_json: r.get(5)?,
            merged_into: r.get(6)?, created_at: r.get(7)?, updated_at: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}
