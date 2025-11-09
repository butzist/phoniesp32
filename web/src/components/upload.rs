use dioxus::html::FileData;
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use dioxus_bulma as b;
use wasm_bindgen::JsCast;
use web_sys::{console, js_sys::JsString};

use crate::metadata;
use crate::services;

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
pub fn Upload(on_complete: Option<EventHandler<()>>) -> Element {
    let mut upload_status = use_signal(|| UploadStatus::NotReady);
    let mut conversion_status = use_signal(|| ConversionStatus::Idle);
    let mut upload_progress = use_signal(|| 0.0f64);

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

        match services::transcoder::transcode(data.to_vec().into(), progress).await {
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
        let result: anyhow::Result<()> = try {
            let file_name = file_name.as_ref().context("file name not set")?;
            let mut data = conversion_status
                .take()
                .take_file_data()
                .context("file data not set")?;

            let edited = edited_metadata().context("metadata not set")?;

            if let Some(original) = metadata() {
                if edited != original {
                    metadata::update_metadata(&mut data, &edited)
                        .await
                        .context("failed updating metadata")?;
                }
            }

            upload_status.set(UploadStatus::Pending);

            services::files::put_file(&file_name, data, Box::new(move |current, total| {
                upload_progress.set(current as f64 / total as f64);
            }))
                .await
                .context("uploading file")?;
        };

        if let Err(err) = result {
            upload_status.set(UploadStatus::Error(format!("{err:?}")))
        } else {
            upload_status.set(UploadStatus::Complete);
            if let Some(on_complete) = on_complete {
                on_complete.call(());
            }
        }
    };

    rsx! {
        b::Section {
            b::Container {
                b::Field {
                    b::Control {
                        div { class: "file",
                            label { class: "file-label",
                                input {
                                    class: "file-input",
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
                                span { class: "file-cta",
                                    span { class: "file-icon",
                                        i { class: "fas fa-upload" }
                                    }
                                    span { class: "file-label", "Choose a file…" }
                                }
                                 span { class: "file-name", "{file_name().as_ref().map(|s| s.as_str()).unwrap_or(\"No file selected\")}" }
                            }
                        }
                    }
                }
                {
                    conversion_status.read().progress_percent().map(|percent| rsx! {
                        b::Field {
                            b::Control {
                                b::Progress { value: percent as f32, max: 100.0, "Transcoding..." }
                            }
                        }
                    })
                }
                {
                    (*upload_status.read() == UploadStatus::Pending).then(|| rsx! {
                        b::Field {
                            b::Control {
                                b::Progress { value: (*upload_progress.read() * 100.0) as f32, max: 100.0, "Uploading..." }
                            }
                        }
                    })
                }
                {
                    matches!(*conversion_status.read(), ConversionStatus::Complete(_)).then(|| rsx! {
                        b::Field {
                            b::Label { "Artist" }
                            b::Control {
                                 b::Input {
                                     value: "{edited_metadata().as_ref().map(|m| m.artist.as_str()).unwrap_or(\"\")}",
                                     oninput: move |e: dioxus::html::FormEvent| {
                                         if let Some(mut m) = edited_metadata() {
                                             m.artist = e.value().chars().take(31).collect::<heapless::String<31>>();
                                             edited_metadata.set(Some(m));
                                         }
                                     },
                                 }
                            }
                        }
                        b::Field {
                            b::Label { "Title" }
                            b::Control {
                                 b::Input {
                                     value: "{edited_metadata().as_ref().map(|m| m.title.as_str()).unwrap_or(\"\")}",
                                     oninput: move |e: dioxus::html::FormEvent| {
                                         if let Some(mut m) = edited_metadata() {
                                             m.title = e.value().chars().take(31).collect::<heapless::String<31>>();
                                             edited_metadata.set(Some(m));
                                         }
                                     },
                                 }
                            }
                        }
                        b::Field {
                            b::Label { "Album" }
                            b::Control {
                                 b::Input {
                                     value: "{edited_metadata().as_ref().map(|m| m.album.as_str()).unwrap_or(\"\")}",
                                     oninput: move |e: dioxus::html::FormEvent| {
                                         if let Some(mut m) = edited_metadata() {
                                             m.album = e.value().chars().take(31).collect::<heapless::String<31>>();
                                             edited_metadata.set(Some(m));
                                         }
                                     },
                                 }
                            }
                        }
                    })
                }
                b::Field {
                    b::Control {
                        b::Button {
                            color: b::BulmaColor::Primary,
                            disabled: *upload_status.read() != UploadStatus::Ready,
                            onclick: move |_| async move { perform_upload().await },
                            "Upload"
                        }
                    }
                }
            }
        }
    }
}
