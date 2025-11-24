use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::utils::{resolve_relative_url, FileMetadata};
use super::REQUEST_TIMEOUT;

#[derive(Debug, Deserialize, Clone)]
pub struct StatusResponse {
    pub position_seconds: u16,
    pub state: PlaybackState,
    pub index_in_playlist: usize,
    pub playlist_name: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CurrentPlaylistResponse {
    pub playlist_name: String,
    pub files: Vec<PlaylistFileResponse>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PlaylistFileResponse {
    pub file: String,
    pub metadata: Option<FileMetadata>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayRequest {
    File(String),
    #[allow(dead_code)]
    Playlist(Vec<String>),
    PlaylistRef(String),
}

pub async fn get_status() -> Result<StatusResponse> {
    let url = resolve_relative_url("/api/playback/status")?;
    let client = reqwest::Client::default();
    let response = client
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("getting playback status")?;
    let status: StatusResponse = response.json().await.context("reading response")?;

    Ok(status)
}

pub async fn get_current_playlist() -> Result<CurrentPlaylistResponse> {
    let url = resolve_relative_url("/api/playback/current_playlist")?;
    let client = reqwest::Client::default();
    let response = client
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("getting current playlist")?;
    let current_playlist: CurrentPlaylistResponse =
        response.json().await.context("reading response")?;

    Ok(current_playlist)
}

pub(crate) async fn play_file(file: &str) -> Result<()> {
    let url = resolve_relative_url("/api/playback/play")?;
    let client = reqwest::Client::default();
    client
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .json(&PlayRequest::File(file.to_string()))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("playing file")?;

    Ok(())
}

#[allow(dead_code)]
pub(crate) async fn play_playlist(files: Vec<String>) -> Result<()> {
    let url = resolve_relative_url("/api/playback/play")?;
    let client = reqwest::Client::default();
    client
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .json(&PlayRequest::Playlist(files))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("playing playlist")?;

    Ok(())
}

pub(crate) async fn play_playlist_ref(playlist_ref: &str) -> Result<()> {
    let url = resolve_relative_url("/api/playback/play")?;
    let client = reqwest::Client::default();
    client
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .json(&PlayRequest::PlaylistRef(playlist_ref.to_string()))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("playing playlist reference")?;

    Ok(())
}

pub async fn stop() -> Result<()> {
    let url = resolve_relative_url("/api/playback/stop")?;
    reqwest::Client::default()
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("stopping playback")?;

    Ok(())
}

pub async fn toggle_pause() -> Result<()> {
    let url = resolve_relative_url("/api/playback/pause")?;
    reqwest::Client::default()
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("toggling pause")?;

    Ok(())
}

pub async fn previous() -> Result<()> {
    let url = resolve_relative_url("/api/playback/previous")?;
    reqwest::Client::default()
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("going to previous track")?;

    Ok(())
}

pub async fn next() -> Result<()> {
    let url = resolve_relative_url("/api/playback/next")?;
    reqwest::Client::default()
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("going to next track")?;

    Ok(())
}

pub async fn volume_up() -> Result<()> {
    let url = resolve_relative_url("/api/playback/volume_up")?;
    reqwest::Client::default()
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("increasing volume")?;

    Ok(())
}

pub async fn volume_down() -> Result<()> {
    let url = resolve_relative_url("/api/playback/volume_down")?;
    reqwest::Client::default()
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .context("decreasing volume")?;

    Ok(())
}
