use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::utils::resolve_relative_url;

#[derive(Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(alias = "SSID")]
    pub ssid: String,

    #[serde(alias = "PASSWORD")]
    pub password: String,
}

pub(crate) async fn put_config(config: &DeviceConfig) -> Result<()> {
    let url = resolve_relative_url("/api/config")?;
    let client = reqwest::Client::default();
    client
        .put(url)
        .json(config)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub(crate) async fn delete_config() -> Result<()> {
    let url = resolve_relative_url("/api/config")?;
    let client = reqwest::Client::default();
    client.delete(url).send().await?.error_for_status()?;

    Ok(())
}
