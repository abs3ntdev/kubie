use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
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

/// Returns the cache directory for a given provider.
/// `$XDG_CACHE_HOME/kubie/cloud/<provider>/` (default `~/.cache/kubie/cloud/<provider>/`)
fn cache_path(provider: &str) -> PathBuf {
    let base = if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(dir)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".cache")
    };
    base.join("kubie").join("cloud").join(provider).join("contexts.json")
}

/// Save discovered contexts for a provider.
pub fn save_contexts(provider: &str, contexts: &[CloudContext]) -> Result<()> {
    let cached: Vec<CachedContext> = contexts
        .iter()
        .map(|c| CachedContext {
            context_name: c.context_name.clone(),
            provider_key: c.provider_key.clone(),
            provider: c.provider.to_string(),
        })
        .collect();
    let file = CacheFile {
        version: CACHE_VERSION,
        contexts: cached,
    };
    ioutil::write_json(cache_path(provider), &file).context("Could not save cloud context cache")
}

/// Map a cached provider name string to its static equivalent.
/// Returns `None` for unknown providers (stale cache from a removed provider).
fn match_provider_name(name: &str) -> Option<&'static str> {
    match name {
        "doctl" => Some("doctl"),
        _ => None,
    }
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
                .filter_map(|c| {
                    let provider = match_provider_name(&c.provider)?;
                    Some(CloudContext {
                        context_name: c.context_name,
                        provider_key: c.provider_key,
                        provider,
                    })
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
