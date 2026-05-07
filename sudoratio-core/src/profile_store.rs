//! Filesystem persistence for user-registered client docs (`<data_dir>/clients/<name>.toml`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub(crate) const SUBDIR: &str = "clients";

pub(crate) fn dir_for(data_dir: &Path) -> PathBuf {
    data_dir.join(SUBDIR)
}

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

pub(crate) async fn save(data_dir: &Path, client: &str, toml_str: &str) -> Result<()> {
    validate_client(client)?;
    let dir = dir_for(data_dir);
    tokio::fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("create clients dir {}", dir.display()))?;
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

pub(crate) async fn delete(data_dir: &Path, client: &str) -> Result<()> {
    validate_client(client)?;
    let path = dir_for(data_dir).join(format!("{client}.toml"));
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("remove {}", path.display())),
    }
}

pub(crate) async fn read_all(data_dir: &Path) -> Result<Vec<(PathBuf, String)>> {
    let dir = dir_for(data_dir);
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("read_dir {}", dir.display())),
    };
    let mut out = Vec::new();
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
        out.push((path, toml));
    }
    Ok(out)
}
