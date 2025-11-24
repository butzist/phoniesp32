use dioxus::html::FileData;
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use dioxus_bulma as b;
use wasm_bindgen::JsCast;

use crate::components::use_toast;
use crate::metadata;
use crate::services;
use crate::services::files::FileExistsAction;

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

#[derive(Debug, Default)]
struct UploadProgress {
    uploaded: u64,
    total: u64,
}

impl UploadProgress {
    fn percent(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.uploaded as f32 / self.total as f32) * 100.0
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
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
    let mut upload_progress = use_signal(UploadProgress::default);
    let mut toast = use_toast();

    let mut input_element: Signal<Option<web_sys::HtmlInputElement>> = use_signal(|| None);
    let mut file_name = use_signal(|| None::<String>);
    let mut selected_files = use_signal(Vec::<FileData>::new);
    let mut computed_name = use_signal(|| None::<String>);
    let mut metadata = use_signal(|| None::<metadata::Metadata>);
    let mut edited_metadata = use_signal(|| None::<metadata::Metadata>);
    let mut file_exists_action = use_signal(|| FileExistsAction::New);

    // Start conversion
    use_effect(move || {
        let Some(file) = selected_files.read().first().cloned() else {
            return;
        };

        spawn(async move {
            // initialize conversion status
            conversion_status.set(ConversionStatus::Running(0));
            metadata.set(None);
            edited_metadata.set(None);
            file_exists_action.set(FileExistsAction::New);
            file_name.set(Some(file.name()));

            // read file data
            let data = match file.read_bytes().await {
                Ok(data) => data.to_vec(),
                Err(err) => {
                    conversion_status
                        // compute file name (hash) and check if already exists
                        .set(ConversionStatus::Error(format!(
                            "Failed to read file: {:?}",
                            err
                        )));
                    return;
                }
            };

            // update status
            let computed = services::files::compute_file_name(&data);
            computed_name.set(Some(computed));

            // start conversion
            let mut conversion_status = conversion_status;
            let progress = move |percent: usize, _total: usize| {
                if ConversionStatus::Running(percent as u8) != *conversion_status.read() {
                    conversion_status.set(ConversionStatus::Running(percent as u8));
                }
            };
            match services::transcoder::transcode(data.into(), progress).await {
                Ok(output) => {
                    let output_extracted = metadata::extract_metadata(&output).await;
                    metadata.set(Some(output_extracted.clone()));
                    edited_metadata.set(Some(output_extracted));
                    conversion_status.set(ConversionStatus::Complete(output));
                }
                Err(err) => {
                    conversion_status.set(ConversionStatus::Error(format!(
                        "Failed to transcode file: {:?}",
                        err
                    )));
                }
            }
        });
    });

    // React on conversion complete
    use_effect(move || {
        if let (Some(computed_name), ConversionStatus::Complete(transcoded_data)) =
            (computed_name.read().cloned(), &*conversion_status.read())
        {
            let transcoded_data_len = transcoded_data.len() as u64;

            spawn(async move {
                match services::files::file_exists_with_size(&computed_name, transcoded_data_len)
                    .await
                {
                    Ok(action) => {
                        upload_status.set(UploadStatus::Ready);
                        file_exists_action.set(action);
                    }
                    Err(_) => {
                        upload_status.set(UploadStatus::Error(
                            "Failed to check transcoded file existence".to_string(),
                        ));
                    }
                }
            });
        }
    });

    // Show toasts for errors
    use_effect({
        move || {
            if let Some(err) = conversion_status.read().error() {
                toast.show_error(err);
            }

            if let Some(err) = upload_status.read().error() {
                toast.show_error(err);
            }
        }
    });

    let mut start_upload = async move || {
        let result: anyhow::Result<()> = try {
            let computed_name = computed_name.read();
            let computed_name = computed_name.as_ref().context("computed name not set")?;
            let mut data = conversion_status
                .take()
                .take_file_data()
                .context("file data not set")?;
            let edited = edited_metadata.read();
            let edited = edited.as_ref().context("metadata not set")?;
            let metadata = metadata.read();
            if let Some(original) = metadata.as_ref() {
                if edited != original {
                    metadata::update_metadata(&mut data, edited)
                        .await
                        .context("failed updating metadata")?;
                }
            }
            upload_status.set(UploadStatus::Pending);

            let total_size = data.len() as u64;
            upload_progress.set(UploadProgress {
                uploaded: 0,
                total: total_size,
            });

            let progress_callback = |uploaded: u64, total: u64| {
                upload_progress.set(UploadProgress { uploaded, total });
            };

            services::files::upload_file_chunked(
                computed_name.as_str(),
                data,
                128 * 1024,
                3,
                progress_callback,
            )
            .await
            .context("uploading file")?;
        };

        if let Err(err) = result {
            toast.show_error(format!("Failed to upload file: {err:?}"));
            upload_status.set(UploadStatus::Ready);
        } else {
            upload_status.set(UploadStatus::Complete);
            toast.show_success("File uploaded successfully!");
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
                                        let files = e.files();
                                        selected_files.set(files);
                                    },
                                }
                                span { class: "file-cta",
                                    span { class: "file-icon",
                                        i { class: "fas fa-upload" }
                                    }
                                    span { class: "file-label", "Choose a file…" }
                                }
                                span { class: "file-name",
                                    "{file_name().as_ref().map(|s| s.as_str()).unwrap_or(\"No file selected\")}"
                                }
                            }
                        }
                    }
                }
                {conversion_status.read().progress_percent().map(|percent| rsx! {
                    b::Field {
                        b::Control {
                            b::Progress { value: percent as f32, max: 100.0, "Transcoding..." }
                        }
                    }
                })}
                {matches!(*upload_status.read(), UploadStatus::Pending).then(|| rsx! {
                    b::Field {
                        b::Control {
                            b::Progress { value: upload_progress.read().percent(), max: 100.0,
                                "Uploading... ({upload_progress.read().uploaded}/{upload_progress.read().total} bytes)"
                            }
                        }
                    }
                })}

                {
                    matches!(*conversion_status.read(), ConversionStatus::Complete(_))
                        .then(|| rsx! {
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
                            color: match *file_exists_action.read() {
                                services::files::FileExistsAction::New => b::BulmaColor::Primary,
                                services::files::FileExistsAction::Continue => b::BulmaColor::Warning,
                                services::files::FileExistsAction::Overwrite => b::BulmaColor::Danger,
                            },
                            disabled: *upload_status.read() != UploadStatus::Ready,
                            loading: *upload_status.read() == UploadStatus::Pending,
                            onclick: move |_| {
                                spawn(async move { start_upload().await });
                            },
                            match *file_exists_action.read() {
                                services::files::FileExistsAction::New => "Upload",
                                services::files::FileExistsAction::Continue => "Upload (continue)",
                                services::files::FileExistsAction::Overwrite => "Upload (overwrite)",
                            }
                        }
                    }
                }
            }
        }
    }
}
