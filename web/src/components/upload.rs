use crate::metadata;
use crate::services::{self, transcoder};

use anyhow::{Context, Result};
use dioxus::html::FileData;
use dioxus::{html::HasFileData, prelude::*, web::WebEventExt as _};
use wasm_bindgen::JsCast;
use web_sys::{console, js_sys::JsString};

#[derive(Debug, PartialEq, Default)]
enum UploadStatus {
    #[default]
    NotReady,
    Ready,
    Pending,
    Complete,
    Error(String),
}

impl UploadStatus {
    fn error(&self) -> Option<&str> {
        match self {
            UploadStatus::Error(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Default)]
enum ConversionStatus {
    #[default]
    Idle,
    Running(u8),
    Complete(Box<[u8]>),
    Error(String),
}

impl ConversionStatus {
    fn progress_percent(&self) -> Option<u8> {
        match self {
            ConversionStatus::Running(p) => Some(*p),
            _ => None,
        }
    }

    fn take_file_data(self) -> Option<Box<[u8]>> {
        match self {
            ConversionStatus::Complete(d) => Some(d),
            _ => None,
        }
    }

    fn error(&self) -> Option<&str> {
        match self {
            ConversionStatus::Error(err) => Some(err),
            _ => None,
        }
    }
}

#[component]
pub fn Upload() -> Element {
    let last_fob_result = use_resource(services::fob::get_last_fob);
    let last_fob_status = use_memo(move || match &*last_fob_result.read() {
        None => "Loading...".to_string(),
        Some(Ok(Some(fob))) => fob.clone(),
        Some(Ok(None)) => "".to_string(),
        Some(Err(err)) => format!("Error: {err:?}"),
    });
    let last_fob = use_memo(move || match &*last_fob_result.read() {
        Some(Ok(Some(fob))) => Some(fob.clone()),
        _ => None,
    });

    let mut upload_status = use_signal(|| UploadStatus::NotReady);
    let mut conversion_status = use_signal(|| ConversionStatus::Idle);

    let mut input_element: Signal<Option<web_sys::HtmlInputElement>> = use_signal(|| None);
    let mut file_name = use_signal(|| None);
    let mut metadata = use_signal(|| None::<metadata::Metadata>);
    let mut edited_metadata = use_signal(|| None::<metadata::Metadata>);

    use_effect(move || {
        if let (Some(_), ConversionStatus::Complete(_)) =
            (&*file_name.read(), &*conversion_status.read())
        {
            upload_status.set(UploadStatus::Ready)
        }
    });

    // temporary solution for error reporting
    // TODO use toast or banner
    use_effect(move || {
        if let Some(err) = conversion_status.read().error() {
            console::log_1(&JsString::from(err));
        }

        if let Some(err) = upload_status.read().error() {
            console::log_1(&JsString::from(err));
        }
    });

    let process_files = move |files: Vec<FileData>| async move {
        let Some(file) = files.first().cloned() else {
            return;
        };
        conversion_status.set(ConversionStatus::Running(0));
        metadata.set(None);
        edited_metadata.set(None);
        file_name.set(Some(file.name()));
        let data = file.read_bytes().await.expect("failed reading file");
        let progress = move |percent: usize, _total: usize| {
            if ConversionStatus::Running(percent as u8) != *conversion_status.read() {
                conversion_status.set(ConversionStatus::Running(percent as u8));
            }
        };

        match transcoder::transcode(data.to_vec().into(), progress).await {
            Ok(output) => {
                let output_extracted = metadata::extract_metadata(&output).await;
                metadata.set(Some(output_extracted.clone()));
                edited_metadata.set(Some(output_extracted));
                conversion_status.set(ConversionStatus::Complete(output));
            }
            Err(err) => conversion_status.set(ConversionStatus::Error(format!("{err:?}"))),
        }
    };

    let perform_upload = move || async move {
        let result: Result<()> = try {
            let file_name = file_name.as_ref().context("file name not set")?;
            let mut data = conversion_status
                .take()
                .take_file_data()
                .context("file data not set")?;
            let last_fob = last_fob().context("fob id missing")?;

            if let (Some(edited), Some(original)) = (edited_metadata(), metadata()) {
                if edited != original {
                    let result = metadata::update_metadata(&mut data, &edited)
                        .await
                        .context("failed updating metadata");
                    if let Err(err) = result {
                        conversion_status.set(ConversionStatus::Error(format!("{err:?}")));
                        conversion_status.set(ConversionStatus::Error(format!("{err:?}")));
                        return;
                    }
                }
            }

            upload_status.set(UploadStatus::Pending);

            services::files::put_file(&file_name, data)
                .await
                .context("uploading file")?;
            services::fob::associate_fob(&last_fob, &file_name)
                .await
                .context("associating fob")?;
        };

        if let Err(err) = result {
            upload_status.set(UploadStatus::Error(format!("{err:?}")))
        } else {
            upload_status.set(UploadStatus::Complete);
        }
    };

    rsx! {
        div { "Last scanned fob: {last_fob_status}" }
        div { class: "fileinput",
            div {
                class: "dropzone",
                id: "dropzone",
                hidden: !matches!(*conversion_status.read(), ConversionStatus::Idle),
                onclick: move |_| {
                    if let Some(input) = input_element() {
                        input.click();
                    }
                },
                ondrop: move |e| {
                    e.prevent_default();
                    spawn(process_files(e.files()));
                },
                ondragover: move |e| {
                    e.prevent_default();
                },
                "Click here or drop audio file"
            }
            input {
                r#type: "file",
                onmounted: move |element| {
                    let element = element.as_web_event();
                    let input = element.dyn_into().ok();
                    input_element.set(input);
                },
                onchange: move |e| {
                    e.prevent_default();
                    spawn(process_files(e.files()));
                },
            }
            div { class: "filename", "{file_name:?}" }
            {
                matches!(*conversion_status.read(), ConversionStatus::Complete(_))
                    .then(|| rsx! {
                        div { class: "metadata",
                            {
                                edited_metadata()
                                    .map(|meta| {
                                        let artist = meta.artist.clone();
                                        let title = meta.title.clone();
                                        let album = meta.album.clone();
                                        let meta_for_artist = meta.clone();
                                        let meta_for_title = meta.clone();
                                        let meta_for_album = meta.clone();
                                        rsx! {
                                            div {
                                                label { "Artist:" }
                                                input {
                                                    r#type: "text",
                                                    value: artist.as_str(),
                                                    maxlength: "31",
                                                    oninput: move |e| {
                                                        let mut new_meta = meta_for_artist.clone();
                                                        new_meta.artist = e.value().chars().take(31).collect();
                                                        edited_metadata.set(Some(new_meta));
                                                    },
                                                }
                                            }
                                            div {
                                                label { "Title:" }
                                                input {
                                                    r#type: "text",
                                                    value: title.as_str(),
                                                    maxlength: "31",
                                                    oninput: move |e| {
                                                        let mut new_meta = meta_for_title.clone();
                                                        new_meta.title = e.value().chars().take(31).collect();
                                                        edited_metadata.set(Some(new_meta));
                                                    },
                                                }
                                            }
                                            div {
                                                label { "Album:" }
                                                input {
                                                    r#type: "text",
                                                    value: album.as_str(),
                                                    maxlength: "31",
                                                    oninput: move |e| {
                                                        let mut new_meta = meta_for_album.clone();
                                                        new_meta.album = e.value().chars().take(31).collect();
                                                        edited_metadata.set(Some(new_meta));
                                                    },
                                                }
                                            }
                                        }
                                    })
                            }
                        }
                    })
            }
        }
        div {
            class: "progress",
            hidden: conversion_status.read().progress_percent().is_none(),
            div {
                class: "bar",
                width: format!("{}%", conversion_status.read().progress_percent().unwrap_or(0)),
            }
        }
        button {
            class: "btn primary",
            disabled: *upload_status.read() != UploadStatus::Ready,
            onclick: move |_| async move { perform_upload().await },
            "Upload"
        }
    }
}
