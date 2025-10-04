use anyhow::{Context, Result};
use reqwest::Method;

use super::utils::resolve_relative_url;
pub(crate) async fn put_file(name: &str, content: Box<[u8]>) -> Result<()> {
    let path = format!("/api/files/{name}");
    let url = resolve_relative_url(&path)?;
    let client = reqwest::Client::default();
    client
        .request(Method::PUT, &url)
        .body(content.to_vec())
        .send()
        .await?
        .error_for_status()
        .context("uploading file")?;

    Ok(())
}
