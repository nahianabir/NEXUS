//! File hashing for content-addressable dedup.

use std::path::Path;
use std::io::Read;

use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub fn hash_file(path: &Path) -> AppResult<(String, u64)> {
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok((hex::encode(hasher.finalize()), total))
}

pub fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

#[allow(dead_code)]
fn _typecheck(_: AppError) {}