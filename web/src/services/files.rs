use anyhow::{Context, Result};
use base32::encode;
use dioxus::core::bail;
use reqwest::{Method, Response, StatusCode};
use sha1::{Digest, Sha1};

use super::utils::{resolve_relative_url, FileEntry};
use super::REQUEST_TIMEOUT;

#[derive(Debug, PartialEq)]
pub enum FileExistsAction {
    New,
    Continue,
    Overwrite,
}

pub(crate) async fn list_files() -> Result<Vec<FileEntry>> {
    let url = resolve_relative_url("/api/files")?;
    let client = reqwest::Client::default();
    let response = client
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(Response::error_for_status)
        .context("listing files")?;
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

async fn create_file(name: &str) -> Result<()> {
    let path = format!("/api/files/{name}");
    let url = resolve_relative_url(&path)?;
    let client = reqwest::Client::default();

    client
        .request(Method::POST, &url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(Response::error_for_status)
        .context("creating file")?;

    Ok(())
}

pub(crate) async fn file_exists_with_size(
    name: &str,
    target_size: u64,
) -> Result<FileExistsAction> {
    let path = format!("/api/files/{name}");
    let url = resolve_relative_url(&path)?;
    let client = reqwest::Client::default();

    let response = client
        .request(Method::HEAD, &url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .context("checking for file")?;

    match response.status() {
        StatusCode::OK => {
            let current_size: u64 = response
                .headers()
                .get("Upload-Offset")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse().ok())
                .context("missing or invalid Upload-Offset header")?;

            if current_size >= target_size {
                Ok(FileExistsAction::Overwrite)
            } else {
                Ok(FileExistsAction::Continue)
            }
        }
        StatusCode::NOT_FOUND => Ok(FileExistsAction::New),
        status => bail!("Unexpected status code: {}", status),
    }
}

pub(crate) async fn get_file_size(name: &str) -> Result<u64> {
    let path = format!("/api/files/{name}");
    let url = resolve_relative_url(&path)?;
    let client = reqwest::Client::default();

    let response = client
        .request(Method::HEAD, &url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(Response::error_for_status)
        .context("getting file size")?;

    response
        .headers()
        .get("Upload-Offset")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok())
        .context("missing or invalid Upload-Offset header")
}

async fn upload_chunk(name: &str, offset: u64, chunk: &[u8], max_retries: u32) -> Result<()> {
    let path = format!("/api/files/{name}");
    let url = resolve_relative_url(&path)?;
    let client = reqwest::Client::default();

    for attempt in 0..=max_retries {
        let response = client
            .request(Method::PATCH, &url)
            .timeout(REQUEST_TIMEOUT)
            .header("Upload-Offset", offset.to_string())
            .body(chunk.to_vec())
            .send()
            .await
            .and_then(Response::error_for_status);

        match response {
            Ok(_) => return Ok(()),
            Err(e) if attempt < max_retries => {
                // Wait before retry (exponential backoff)
                let delay_ms = 100 * (2_u32.pow(attempt));
                async_std::task::sleep(std::time::Duration::from_millis(delay_ms as u64)).await;
                continue;
            }
            Err(e) => return Err(e).context("uploading chunk"),
        }
    }

    bail!("Failed to upload chunk after {} retries", max_retries)
}

pub(crate) async fn upload_file_chunked<F>(
    name: &str,
    content: Box<[u8]>,
    chunk_size: usize,
    max_retries: u32,
    mut progress_callback: F,
) -> Result<()>
where
    F: FnMut(u64, u64), // (bytes_uploaded, total_bytes)
{
    let total_size = content.len() as u64;

    // Check if file already exists and determine action
    let file_exists_action = file_exists_with_size(name, total_size).await?;

    let mut uploaded = match file_exists_action {
        FileExistsAction::New | FileExistsAction::Overwrite => {
            // File doesn't exist, create it first - or already exists and should be overwritten
            create_file(name).await.context("creating empty file")?;
            progress_callback(0, total_size);
            0
        }
        FileExistsAction::Continue => {
            // File exists and can be resumed, get current size
            let current_size = get_file_size(name).await?;
            progress_callback(current_size, total_size);
            current_size
        }
    };

    // Upload chunks
    while uploaded < total_size {
        let start = uploaded as usize;
        let end = std::cmp::min(start + chunk_size, content.len());
        let chunk = &content[start..end];

        upload_chunk(name, uploaded, chunk, max_retries)
            .await
            .context("uploading chunk")?;

        uploaded += chunk.len() as u64;
        progress_callback(uploaded, total_size);
    }

    Ok(())
}
