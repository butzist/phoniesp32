use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::utils::{resolve_relative_url, FileEntry};

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
    file: &'a str,
}

pub(crate) async fn get_last_fob() -> Result<Option<String>> {
    let url = resolve_relative_url("/api/last_fob")?;
    let response = reqwest::get(url).await?.error_for_status()?;
    let fob: LastFob = response.json().await.context("reading response")?;

    Ok(fob.last_fob)
}

pub(crate) async fn list_associations() -> Result<Vec<Association>> {
    let url = resolve_relative_url("/api/associations")?;
    let response = reqwest::get(url).await?.error_for_status()?;
    let associations: Vec<Association> = response.json().await.context("reading response")?;

    Ok(associations)
}

pub(crate) async fn associate_fob(fob: &str, file: &str) -> Result<()> {
    let url = resolve_relative_url("/api/associations")?;
    let client = reqwest::Client::default();
    client
        .post(url)
        .json(&AssociationRequest { fob, file })
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
