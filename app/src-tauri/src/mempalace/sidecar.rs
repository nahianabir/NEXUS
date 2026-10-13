//! MemPalace sidecar. The Python tool is expected to expose a tiny
//! JSON-RPC over stdin/stdout. We spawn one process per call to keep
//! the surface small and predictable, with a timeout that defaults to
//! 10 seconds.
//!
//! The exact RPC surface here is a NEXUS-side contract; the real
//! MemPalace binding is documented in `docs/MEMPALACE_INTEGRATION.md`.
//! The integration tests are gated behind `--ignored` until the real
//! MemPalace Python package is installed.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::adapter::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemPalaceConfig {
    /// Absolute path to the MemPalace binary or `mempalace` for PATH lookup.
    pub binary: String,
    /// Optional working directory for the sidecar.
    pub cwd: Option<String>,
    /// Per-call timeout in milliseconds.
    pub timeout_ms: u64,
    /// Optional extra arguments to pass before the JSON-RPC payload.
    pub extra_args: Vec<String>,
}

impl Default for MemPalaceConfig {
    fn default() -> Self {
        Self {
            binary: "mempalace".into(),
            cwd: None,
            timeout_ms: 10_000,
            extra_args: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum RpcRequest {
    #[serde(rename = "health")]
    Health,
    #[serde(rename = "search")]
    Search { query: String, project_id: Option<String>, limit: usize },
    #[serde(rename = "store")]
    Store { record: AdapterMemory },
    #[serde(rename = "delete")]
    Delete { id: String },
    #[serde(rename = "list_collections")]
    ListCollections,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "ok")]
pub enum RpcResponse {
    #[serde(rename = "true")]
    Ok(serde_json::Value),
    #[serde(rename = "false")]
    Err { error: String },
}

pub struct MemPalaceBackend {
    cfg: MemPalaceConfig,
}

impl MemPalaceBackend {
    pub fn new(cfg: MemPalaceConfig) -> Self { Self { cfg } }

    fn call(&self, req: &RpcRequest) -> AdapterResult<RpcResponse> {
        let payload = serde_json::to_string(req)
            .map_err(|e| AdapterError::Upstream(format!("encode: {e}")))?;
        let mut cmd = Command::new(&self.cfg.binary);
        if let Some(dir) = &self.cfg.cwd { cmd.current_dir(dir); }
        cmd.args(&self.cfg.extra_args);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| AdapterError::Io(format!("spawn: {e}")))?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(payload.as_bytes()).map_err(|e| AdapterError::Io(format!("stdin: {e}")))?;
            stdin.write_all(b"\n").map_err(|e| AdapterError::Io(format!("stdin: {e}")))?;
        }
        let timeout = Duration::from_millis(self.cfg.timeout_ms.max(100));
        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        return Err(AdapterError::Upstream("timeout".into()));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(AdapterError::Io(format!("wait: {e}"))),
            }
        }
        let mut out = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            let _ = stdout.read_to_string(&mut out);
        }
        let response: RpcResponse = serde_json::from_str(&out)
            .map_err(|e| AdapterError::Upstream(format!("decode: {e}; payload: {out}")))?;
        Ok(response)
    }
}

impl MemoryBackend for MemPalaceBackend {
    fn kind(&self) -> BackendKind { BackendKind::MemPalace }

    fn health(&self) -> AdapterResult<AdapterDiagnostics> {
        match self.call(&RpcRequest::Health) {
            Ok(RpcResponse::Ok(v)) => Ok(AdapterDiagnostics {
                backend: "mempalace".into(),
                healthy: true,
                version: v.get("version").and_then(|x| x.as_str()).map(|s| s.to_string()),
                message: v.get("message").and_then(|x| x.as_str()).unwrap_or("ok").to_string(),
            }),
            Ok(RpcResponse::Err { error }) => Ok(AdapterDiagnostics {
                backend: "mempalace".into(),
                healthy: false,
                version: None,
                message: error,
            }),
            Err(e) => Ok(AdapterDiagnostics {
                backend: "mempalace".into(),
                healthy: false,
                version: None,
                message: e.to_string(),
            }),
        }
    }

    fn search(&self, query: &str, project_id: Option<&str>, limit: usize) -> AdapterResult<Vec<AdapterMemory>> {
        let req = RpcRequest::Search { query: query.to_string(), project_id: project_id.map(|s| s.to_string()), limit };
        match self.call(&req)? {
            RpcResponse::Ok(v) => serde_json::from_value(v)
                .map_err(|e| AdapterError::Upstream(format!("decode search: {e}"))),
            RpcResponse::Err { error } => Err(AdapterError::Upstream(error)),
        }
    }

    fn store(&self, record: &AdapterMemory) -> AdapterResult<String> {
        let req = RpcRequest::Store { record: record.clone() };
        match self.call(&req)? {
            RpcResponse::Ok(v) => v.get("id").and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| AdapterError::Upstream("missing id".into())),
            RpcResponse::Err { error } => Err(AdapterError::Upstream(error)),
        }
    }

    fn delete(&self, id: &str) -> AdapterResult<()> {
        let req = RpcRequest::Delete { id: id.to_string() };
        match self.call(&req)? {
            RpcResponse::Ok(_) => Ok(()),
            RpcResponse::Err { error } => Err(AdapterError::Upstream(error)),
        }
    }

    fn list_collections(&self) -> AdapterResult<Vec<AdapterCollection>> {
        match self.call(&RpcRequest::ListCollections)? {
            RpcResponse::Ok(v) => serde_json::from_value(v)
                .map_err(|e| AdapterError::Upstream(format!("decode collections: {e}"))),
            RpcResponse::Err { error } => Err(AdapterError::Upstream(error)),
        }
    }
}
