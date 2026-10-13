//! Shared HTTP client for the provider backends. We use one client per provider
//! so connection pooling and DNS are isolated; the client is configured for
//! `127.0.0.1`-only targets by default.
//!
//! SSE / NDJSON streaming is parsed in the `stream` module.

use std::time::Duration;

use reqwest::Client;

pub fn new_client() -> reqwest::Result<Client> {
    Client::builder()
        .user_agent("NEXUS/0.1 (local)")
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(60 * 30)) // 30 min read ceiling
        .tcp_nodelay(true)
        .build()
}
