//! Seed bundled client docs on server startup, then activate a sensible default.

use sudoratio_core::profile::BUNDLED_CLIENTS;
use sudoratio_core::{ClientProfileId, Engine};

const DEFAULT_PROFILE: &str = "deluge@2.2.0";

pub async fn seed_profiles(core: &Engine) -> anyhow::Result<()> {
    if BUNDLED_CLIENTS.is_empty() {
        anyhow::bail!("no bundled clients compiled in");
    }
    for (client, toml) in BUNDLED_CLIENTS {
        core.register_builtin_client(toml)
            .await
            .map_err(|e| anyhow::anyhow!("{client}: {e}"))?;
    }
    activate_default_or_fallback(core).await?;
    Ok(())
}

/// Activate the bundled default if present, otherwise the first available variant. Used both at
/// startup and after a delete that removed the active profile.
pub async fn activate_default_or_fallback(core: &Engine) -> anyhow::Result<()> {
    let default_id = ClientProfileId::from(DEFAULT_PROFILE);
    if core.set_active_profile(default_id).await.is_ok() {
        tracing::info!("active client profile = {DEFAULT_PROFILE}");
        return Ok(());
    }
    let rows = core.list_profiles().await;
    let Some(first) = rows.first() else {
        anyhow::bail!("no client profile available to activate");
    };
    core.set_active_profile(ClientProfileId::from(first.id.as_str()))
        .await
        .map_err(|e| anyhow::anyhow!("set active profile: {e}"))?;
    tracing::info!(
        profile_id = %first.id,
        "default {DEFAULT_PROFILE} missing; activated first available profile"
    );
    Ok(())
}
