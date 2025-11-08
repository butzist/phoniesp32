use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::utils::{resolve_relative_url, FileMetadata};

#[derive(Debug, Deserialize)]
pub struct Status {
    pub state: State,
    pub position_seconds: Option<u16>,
    pub metadata: Option<FileMetadata>,
}

#[derive(Debug, Deserialize)]
pub enum State {
    Playing,
    Paused,
    Stopped,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayRequest {
    File(String),
    Playlist(Vec<String>),
    PlaylistRef(String),
}

pub(crate) async fn get_status() -> Result<Status> {
    let url = resolve_relative_url("/api/playback/status")?;
    let response = reqwest::get(url).await?.error_for_status()?;
    let status: Status = response.json().await.context("reading response")?;

    Ok(status)
}

pub(crate) async fn play_file(file: &str) -> Result<()> {
    let url = resolve_relative_url("/api/playback/play")?;
    let client = reqwest::Client::default();
    client
        .post(url)
        .json(&PlayRequest::File(file.to_string()))
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub(crate) async fn play_playlist(files: Vec<String>) -> Result<()> {
    let url = resolve_relative_url("/api/playback/play")?;
    let client = reqwest::Client::default();
    client
        .post(url)
        .json(&PlayRequest::Playlist(files))
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub(crate) async fn play_playlist_ref(playlist_ref: &str) -> Result<()> {
    let url = resolve_relative_url("/api/playback/play")?;
    let client = reqwest::Client::default();
    client
        .post(url)
        .json(&PlayRequest::PlaylistRef(playlist_ref.to_string()))
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub(crate) async fn stop() -> Result<()> {
    let url = resolve_relative_url("/api/playback/stop")?;
    reqwest::Client::default()
        .post(url)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub(crate) async fn pause() -> Result<()> {
    let url = resolve_relative_url("/api/playback/pause")?;
    reqwest::Client::default()
        .post(url)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub(crate) async fn volume_up() -> Result<()> {
    let url = resolve_relative_url("/api/playback/volume_up")?;
    reqwest::Client::default()
        .post(url)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub(crate) async fn volume_down() -> Result<()> {
    let url = resolve_relative_url("/api/playback/volume_down")?;
    reqwest::Client::default()
        .post(url)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}