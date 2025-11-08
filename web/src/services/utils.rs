use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FileMetadata {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub duration: u32,
}

#[derive(Debug, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub metadata: FileMetadata,
}

pub(crate) fn resolve_relative_url(path: &str) -> anyhow::Result<String> {
    let window = web_sys::window().ok_or_else(|| anyhow::anyhow!("no window"))?;
    let location = window.location();
    let base = location.origin().map_err(|e| anyhow::anyhow!("{:?}", e))?;
    Ok(format!("{base}{path}"))
}
