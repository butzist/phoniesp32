use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::utils::{resolve_relative_url, FileEntry};
use super::REQUEST_TIMEOUT;

#[derive(Debug, Deserialize)]
struct LastFob {
    last_fob: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Association {
    pub fob: String,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Serialize)]
struct AssociationRequest<'a> {
    fob: &'a str,
    files: &'a [String],
}

pub(crate) async fn get_last_fob() -> Result<Option<String>> {
    let url = resolve_relative_url("/api/last_fob")?;
    let client = reqwest::Client::default();
    let response = client
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("getting last fob")?;
    let fob: LastFob = response.json().await.context("reading response")?;

    Ok(fob.last_fob)
}

pub(crate) async fn list_associations() -> Result<Vec<Association>> {
    let url = resolve_relative_url("/api/associations")?;
    let client = reqwest::Client::default();
    let response = client
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("listing associations")?;
    let associations: Vec<Association> = response.json().await.context("reading response")?;

    Ok(associations)
}

pub(crate) async fn associate_fob(fob: &str, files: &[String]) -> Result<()> {
    let url = resolve_relative_url("/api/associations")?;
    let client = reqwest::Client::default();
    client
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .json(&AssociationRequest { fob, files })
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("associating fob")?;

    Ok(())
}
