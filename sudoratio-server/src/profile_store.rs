//! Filesystem persistence for user-registered client docs.
//!
//! One TOML file per client family at `<config_dir>/client-profiles/<client>.toml`. On startup
//! [`load_all`] walks the directory and re-registers each doc.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sudoratio_core::Engine;

const SUBDIR: &str = "clients";

fn dir_for(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(SUBDIR)
}

/// Restrict client names to a safe filesystem charset.
fn validate_client(client: &str) -> Result<()> {
    if client.is_empty() {
        anyhow::bail!("client name is empty");
    }
    if !client
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        anyhow::bail!("client name {client:?} contains characters outside [A-Za-z0-9._-]");
    }
    Ok(())
}

pub async fn save(config_path: &Path, client: &str, toml_str: &str) -> Result<()> {
    validate_client(client)?;
    let dir = dir_for(config_path);
    tokio::fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("create client-profiles dir {}", dir.display()))?;
    let final_path = dir.join(format!("{client}.toml"));
    let tmp = dir.join(format!(".{client}.toml.tmp"));
    tokio::fs::write(&tmp, toml_str)
        .await
        .with_context(|| format!("write {}", tmp.display()))?;
    tokio::fs::rename(&tmp, &final_path)
        .await
        .with_context(|| format!("rename {} -> {}", tmp.display(), final_path.display()))?;
    Ok(())
}

pub async fn delete(config_path: &Path, client: &str) -> Result<()> {
    validate_client(client)?;
    let path = dir_for(config_path).join(format!("{client}.toml"));
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("remove {}", path.display())),
    }
}

pub async fn load_all(config_path: &Path, core: &Engine) -> Result<usize> {
    let dir = dir_for(config_path);
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e).with_context(|| format!("read_dir {}", dir.display())),
    };
    let mut count = 0;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let toml = match tokio::fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "skipping unreadable client doc");
                continue;
            }
        };
        match core.register_client(&toml).await {
            Ok(ids) => {
                tracing::info!(
                    variants = ids.len(),
                    path = %path.display(),
                    "loaded user client doc"
                );
                count += ids.len();
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "skipping invalid client doc");
            }
        }
    }
    Ok(count)
}
