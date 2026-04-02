use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use dirs;
use serde::{Deserialize, Serialize};

use super::CloudContext;
use crate::ioutil;

/// Current cache schema version. Bump when `CachedContext` changes.
const CACHE_VERSION: u32 = 1;

/// Serializable form of a cloud context for the cache.
#[derive(Serialize, Deserialize)]
struct CachedContext {
    context_name: String,
    provider_key: String,
    provider: String,
}

/// Versioned cache envelope.
#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    contexts: Vec<CachedContext>,
}

/// Returns the cache file path for a given provider.
fn cache_path(provider: &str) -> PathBuf {
    let base = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    base.join("kubie").join("cloud").join(provider).join("contexts.json")
}

/// Save discovered contexts for a provider.
pub fn save_contexts(provider: &str, contexts: &[CloudContext]) -> Result<()> {
    let cached: Vec<CachedContext> = contexts
        .iter()
        .map(|c| CachedContext {
            context_name: c.context_name.clone(),
            provider_key: c.provider_key.clone(),
            provider: c.provider.clone(),
        })
        .collect();
    let file = CacheFile {
        version: CACHE_VERSION,
        contexts: cached,
    };
    ioutil::write_json(cache_path(provider), &file).context("Could not save cloud context cache")
}

/// Load cached contexts for a provider. Returns `None` if no cache exists
/// or the schema version doesn't match.
pub fn load_contexts(provider: &str) -> Result<Option<Vec<CloudContext>>> {
    let path = cache_path(provider);
    if !path.exists() {
        return Ok(None);
    }
    let file: std::result::Result<CacheFile, _> = ioutil::read_json(&path);
    match file {
        Ok(f) if f.version == CACHE_VERSION => {
            let contexts = f
                .contexts
                .into_iter()
                .map(|c| CloudContext {
                    context_name: c.context_name,
                    provider_key: c.provider_key,
                    provider: c.provider,
                })
                .collect();
            Ok(Some(contexts))
        }
        _ => {
            let _ = fs::remove_file(&path);
            Ok(None)
        }
    }
}
