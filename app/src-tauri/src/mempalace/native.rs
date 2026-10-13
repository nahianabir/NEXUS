//! The native backend: a thin adapter over the in-process `memories_v2`
//! store. Always available, requires no installation.

use crate::db::SqlitePool;
use crate::memory_v2;
use super::adapter::*;

pub struct NativeBackend {
    pool: SqlitePool,
}

impl NativeBackend {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }
}

impl MemoryBackend for NativeBackend {
    fn kind(&self) -> BackendKind { BackendKind::Native }

    fn health(&self) -> AdapterResult<AdapterDiagnostics> {
        Ok(AdapterDiagnostics {
            backend: "native".into(),
            healthy: true,
            version: Some("v2".into()),
            message: "in-process memories_v2 store".into(),
        })
    }

    fn search(&self, query: &str, project_id: Option<&str>, limit: usize) -> AdapterResult<Vec<AdapterMemory>> {
        let ids = memory_v2::search::search(&self.pool, query, None, project_id, limit)
            .map_err(|e| AdapterError::Upstream(e.to_string()))?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Ok(m) = memory_v2::memory::get(&self.pool, &id) {
                out.push(AdapterMemory {
                    id: m.id.clone(),
                    backend: "native".into(),
                    project_id: m.project_id,
                    workspace_id: m.workspace_id,
                    category: m.category,
                    content: m.content,
                    source_kind: m.source_kind,
                    source_id: m.source_id,
                    source_excerpt: m.source_excerpt,
                    event_at: m.event_at,
                    valid_from: m.valid_from,
                    valid_until: m.valid_until,
                    created_at: m.created_at,
                    updated_at: m.updated_at,
                });
            }
        }
        Ok(out)
    }

    fn store(&self, record: &AdapterMemory) -> AdapterResult<String> {
        let now = chrono::Utc::now().timestamp_millis();
        let norm = memory_v2::normalize_content(&record.content);
        let hash = memory_v2::content_hash(&norm);
        if let Some(existing) = memory_v2::memory::find_by_hash(&self.pool, &hash)
            .map_err(|e| AdapterError::Upstream(e.to_string()))?
        {
            return Ok(existing.id);
        }
        let m = memory_v2::memory::MemoryV2 {
            id: memory_v2::memory::new_id(),
            workspace_id: record.workspace_id.clone(),
            project_id: record.project_id.clone(),
            conversation_id: None,
            category: record.category.clone(),
            content: record.content.clone(),
            content_hash: hash,
            normalized_content: norm,
            source_kind: record.source_kind.clone(),
            source_id: record.source_id.clone(),
            source_label: None,
            source_excerpt: record.source_excerpt.clone(),
            event_at: record.event_at,
            valid_from: record.valid_from,
            valid_until: record.valid_until,
            verified_at: Some(now),
            superseded_by: None,
            deleted_at: None,
            approved: true,
            verification: "approved".into(),
            pinned: false,
            created_at: now,
            updated_at: now,
        };
        memory_v2::memory::insert(&self.pool, &m)
            .map_err(|e| AdapterError::Upstream(e.to_string()))?;
        Ok(m.id)
    }

    fn delete(&self, id: &str) -> AdapterResult<()> {
        memory_v2::memory::soft_delete(&self.pool, id)
            .map_err(|e| AdapterError::Upstream(e.to_string()))?;
        Ok(())
    }

    fn list_collections(&self) -> AdapterResult<Vec<AdapterCollection>> {
        let list = crate::projects::projects::list(&self.pool, false)
            .map_err(|e| AdapterError::Upstream(e.to_string()))?;
        Ok(list.into_iter().map(|p| AdapterCollection {
            id: p.id, name: p.name, backend: "native".into(),
        }).collect())
    }
}
