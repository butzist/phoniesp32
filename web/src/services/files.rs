use anyhow::{Context, Result};
use base32::encode;
use dioxus::core::bail;
use reqwest::{Method, StatusCode};
use sha1::{Digest, Sha1};

use super::utils::{resolve_relative_url, FileEntry};

pub(crate) async fn list_files() -> Result<Vec<FileEntry>> {
    let url = resolve_relative_url("/api/files")?;
    let response = reqwest::get(url).await?.error_for_status()?;
    let files: Vec<FileEntry> = response.json().await.context("reading response")?;

    Ok(files)
}

pub(crate) fn compute_file_name(content: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(content);
    let hash = hasher.finalize();
    let encoded = encode(base32::Alphabet::RFC4648 { padding: false }, &hash);
    encoded.chars().take(8).collect()
}

pub(crate) async fn file_exists(name: &str) -> Result<bool> {
    let path = format!("/api/files/{name}");
    let url = resolve_relative_url(&path)?;
    let response = reqwest::get(url).await?;

    match response.status() {
        StatusCode::OK => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        status => bail!("Unexpected status code: {}", status),
    }
}

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
