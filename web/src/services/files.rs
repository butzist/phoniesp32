use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{channel::mpsc::channel, SinkExt};
use futures::{stream, try_join, StreamExt};
use reqwest::Method;

use super::utils::{resolve_relative_url, FileEntry};

pub(crate) async fn list_files() -> Result<Vec<FileEntry>> {
    let url = resolve_relative_url("/api/files")?;
    let response = reqwest::get(url).await?.error_for_status()?;
    let files: Vec<FileEntry> = response.json().await.context("reading response")?;

    Ok(files)
}

pub(crate) async fn put_file(
    name: &str,
    content: Box<[u8]>,
    mut progress: impl FnMut(usize, usize),
) -> Result<()> {
    let total = content.len();
    progress(0, total);

    let (tx, mut rx) = channel::<usize>(1);
    let stream = stream::unfold(
        (content, 0, tx),
        move |(content, mut sent, mut tx)| async move {
            let end = (sent + 8192).min(total);
            let buf = &content[sent..end];

            sent = end;
            tx.send(sent).await.ok()?;

            Some((
                Ok::<Bytes, std::io::Error>(Bytes::from(buf.to_vec())),
                (content, sent, tx),
            ))
        },
    );

    let body = reqwest::Body::wrap_stream(stream);

    let path = format!("/api/files/{name}");
    let url = resolve_relative_url(&path)?;
    let client = reqwest::Client::default();

    let receiver = async move {
        while let Some(sent) = rx.next().await {
            progress(sent, total);
        }

        anyhow::Ok(())
    };

    let sender = async move {
        client
            .request(Method::PUT, &url)
            .body(body)
            .send()
            .await?
            .error_for_status()
            .context("uploading file")?;
        anyhow::Ok(())
    };

    try_join!(sender, receiver)?;
    Ok(())
}
