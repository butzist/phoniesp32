use std::sync::Arc;

use crate::services::{self, transcoder};

use anyhow::{Context, Result};
use dioxus::{
    html::{FileEngine, HasFileData},
    prelude::*,
    web::WebEventExt as _,
};
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

    let process_files = move |fileengine: Arc<dyn FileEngine>| async move {
        let Some(name) = fileengine.files().first().cloned() else {
            return;
        };
        conversion_status.set(ConversionStatus::Running(0));

        file_name.set(Some(name.to_string()));

        let data = fileengine
            .read_file(&name)
            .await
            .expect("failed reading file");
        let progress = move |percent: usize, _total: usize| {
            if ConversionStatus::Running(percent as u8) != *conversion_status.read() {
                conversion_status.set(ConversionStatus::Running(percent as u8));
            }
        };

        match transcoder::transcode(data.into(), progress).await {
            Ok(output) => conversion_status.set(ConversionStatus::Complete(output)),
            Err(err) => conversion_status.set(ConversionStatus::Error(format!("{err:?}"))),
        }
    };

    let perform_upload = move || async move {
        let result: Result<()> = try {
            let file_name = file_name.as_ref().context("file name not set")?;
            let file_data = conversion_status
                .take()
                .take_file_data()
                .context("file data not set")?;
            let last_fob = last_fob().context("fob id missing")?;

            upload_status.set(UploadStatus::Pending);

            services::files::put_file(&file_name, file_data)
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
        div {
            "Last scanned fob: {last_fob_status}"
        }
        div { class: "fileinput",
            div { class: "dropzone",
                id: "dropzone",
                hidden: !matches!(*conversion_status.read(), ConversionStatus::Idle),

                onclick: move |_| {
                    if let Some(input) = input_element() {
                            input.click();
                    }
                },
                ondrop: move |e| {
                    e.prevent_default();
                    if let Some(fileengine) = e.data().files() {
                        spawn(process_files(fileengine));
                    }
                },
                ondragover: move |e| {
                    e.prevent_default();
                },

                "Click here or drop audio file"
            }
            input { type: "file",
                onmounted: move |element| {
                    let element = element.as_web_event();
                    let input = element.dyn_into().ok();

                    input_element.set(input);
                },
                onchange: move |e| {
                    e.prevent_default();
                    if let Some(fileengine) = e.data().files() {
                        spawn(process_files(fileengine));
                    }
                },
            }
            div { class: "filename", "{file_name:?}" }
        }
        div { class: "progress", hidden: conversion_status.read().progress_percent().is_none(),
            div { class: "bar",
                width: format!("{}%", conversion_status.read().progress_percent().unwrap_or(0)),
            }
        }
        button { class: "btn primary",
            disabled: *upload_status.read() != UploadStatus::Ready,

            onclick: move |_| async move { perform_upload().await },

            "Upload",
        }
    }
}
