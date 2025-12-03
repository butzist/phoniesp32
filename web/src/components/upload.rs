use dioxus::html::FileData;
use dioxus::prelude::*;
use dioxus_bulma as b;

use crate::components::use_toast;
use crate::metadata;
use crate::services;
use crate::services::files::FileExistsAction;

#[derive(Debug, PartialEq, Default)]
enum UploadStatus {
    #[default]
    NotReady,
    Ready(FileExistsAction),
    Running(UploadProgress),
    Error(String),
}

impl UploadStatus {
    fn progress_percent(&self) -> Option<f32> {
        match self {
            UploadStatus::Running(p) => Some(p.percent()),
            _ => None,
        }
    }

    fn error(&self) -> Option<&str> {
        match self {
            UploadStatus::Error(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
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
    Complete(ConversionResult),
    Error(String),
}

impl ConversionStatus {
    fn progress_percent(&self) -> Option<f32> {
        match self {
            ConversionStatus::Running(p) => Some(*p as f32),
            _ => None,
        }
    }

    fn take_result(self) -> Option<ConversionResult> {
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

#[derive(Debug, Default, Clone, PartialEq)]
struct ConversionResult {
    name: String,
    data: Box<[u8]>,
}

#[component]
pub fn Upload(on_complete: Option<EventHandler<()>>) -> Element {
    let mut toast = use_toast();
    let mut upload_status = use_signal(|| UploadStatus::NotReady);
    let mut conversion_status = use_signal(|| ConversionStatus::Idle);

    let mut file_name = use_signal(|| None::<String>);
    let mut selected_files = use_signal(Vec::<FileData>::new);
    let mut metadata = use_signal(|| None::<metadata::Metadata>);
    let mut edited_metadata = use_signal(|| None::<metadata::Metadata>);

    // Start conversion
    use_effect(move || {
        let Some(file) = selected_files.read().first().cloned() else {
            return;
        };

        spawn(async move {
            // initialize conversion status
            conversion_status.set(ConversionStatus::Running(0));
            upload_status.set(UploadStatus::default());
            metadata.set(None);
            edited_metadata.set(None);
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
                    conversion_status.set(ConversionStatus::Complete(ConversionResult {
                        name: computed,
                        data: output,
                    }));
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

    let check_target_file_exists = move || {
        if let ConversionStatus::Complete(ConversionResult {
            name: computed_name,
            data: transcoded_data,
        }) = &*conversion_status.read()
        {
            let transcoded_data_len = transcoded_data.len() as u64;
            let computed_name = computed_name.clone();

            spawn(async move {
                match services::files::file_exists_with_size(&computed_name, transcoded_data_len)
                    .await
                {
                    Ok(action) => {
                        upload_status.set(UploadStatus::Ready(action));
                    }
                    Err(_) => {
                        upload_status.set(UploadStatus::Error(
                            "Failed to check transcoded file existence".to_string(),
                        ));
                    }
                }
            });
        }
    };

    // React on conversion complete
    use_effect(move || {
        check_target_file_exists();
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
        let Some(mut conversion_result) = conversion_status.take().take_result() else {
            return;
        };

        let result: anyhow::Result<()> = try {
            let edited = edited_metadata.read();
            let edited = edited.as_ref().context("metadata not set")?;
            let metadata = metadata.read();
            if let Some(original) = metadata.as_ref() {
                if edited != original {
                    metadata::update_metadata(&mut conversion_result.data, edited)
                        .await
                        .context("failed updating metadata")?;
                }
            }

            let total_size = conversion_result.data.len() as u64;
            upload_status.set(UploadStatus::Running(UploadProgress {
                uploaded: 0,
                total: total_size,
            }));

            let progress_callback = |uploaded: u64, total: u64| {
                upload_status.set(UploadStatus::Running(UploadProgress { uploaded, total }));
            };

            services::files::upload_file_chunked(
                conversion_result.name.as_str(),
                conversion_result.data.clone(),
                128 * 1024,
                3,
                progress_callback,
            )
            .await
            .context("uploading file")?;
        };

        if let Err(err) = result {
            toast.show_error(format!("Failed to upload file: {err:?}"));
            conversion_status.set(ConversionStatus::Complete(conversion_result));
            check_target_file_exists();
        } else {
            upload_status.set(UploadStatus::default());
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
                            b::Progress { value: percent, max: 100.0, "Transcoding..." }
                        }
                    }
                })}
                {upload_status.read().progress_percent().map(|percent| rsx! {
                    b::Field {
                        b::Control {
                            b::Progress { value: percent, max: 100.0, "Uploading..." }
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
                            color: match *upload_status.read() {
                                UploadStatus::Ready(FileExistsAction::Continue) => b::BulmaColor::Warning,
                                UploadStatus::Ready(FileExistsAction::Overwrite) => b::BulmaColor::Danger,
                                _ => b::BulmaColor::Primary,
                            },
                            disabled: !matches!(*upload_status.read(), UploadStatus::Ready(_)),
                            loading: matches!(*upload_status.read(), UploadStatus::Running(_)),
                            onclick: move |_| {
                                spawn(async move { start_upload().await });
                            },
                            match *upload_status.read() {
                                UploadStatus::Ready(FileExistsAction::Continue) => "Upload (continue)",
                                UploadStatus::Ready(FileExistsAction::Overwrite) => "Upload (overwrite)",
                                _ => "Upload",
                            }
                        }
                    }
                }
            }
        }
    }
}
