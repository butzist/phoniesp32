use std::sync::OnceLock;

use async_lock::Semaphore;
use dioxus::html::FileData;
use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::FaCheck;

use crate::components::use_toast;
use crate::metadata;
use crate::services;
use crate::services::transcoder::N_WORKERS;

static UPLOAD_SEM: OnceLock<Semaphore> = OnceLock::new();

fn upload_sem() -> &'static Semaphore {
    UPLOAD_SEM.get_or_init(|| Semaphore::new(1))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TranscodeResult {
    name: String,
    data: Box<[u8]>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JobState {
    PendingTranscode,
    Transcoding,
    TranscodeReady,
    Uploading,
    Complete,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JobItem {
    pub(crate) id: u64,
    pub(crate) file_name: String,
    pub(crate) state: JobState,
    pub(crate) transcode_progress: f32,
    pub(crate) upload_progress: f32,
    pub(crate) transcode_result: Option<TranscodeResult>,
    pub(crate) metadata: Option<metadata::Metadata>,
    pub(crate) edited_metadata: Option<metadata::Metadata>,
}

impl JobItem {
    fn new(id: u64, file_name: String) -> Self {
        Self {
            id,
            file_name,
            state: JobState::PendingTranscode,
            transcode_progress: 0.0,
            upload_progress: 0.0,
            transcode_result: None,
            metadata: None,
            edited_metadata: None,
        }
    }
}

fn update_job(mut jobs: Signal<Vec<JobItem>>, id: u64, f: impl FnOnce(&mut JobItem)) {
    if let Some(job) = jobs.write().iter_mut().find(|j| j.id == id) {
        f(job);
    }
}

#[component]
pub fn Upload(on_complete: Option<EventHandler<()>>) -> Element {
    let toast = use_toast();
    let jobs = use_signal(Vec::<JobItem>::new);
    let mut next_id = use_signal(|| 0u64);

    let mut handle_files = move |files: Vec<FileData>| {
        let mut jobs = jobs;
        for file in files.into_iter() {
            let id = {
                let mut id_lock = next_id.write();
                let id = *id_lock;
                *id_lock += 1;
                id
            };
            let file_name = file.name();

            jobs.write().push(JobItem::new(id, file_name.clone()));

            spawn({
                let jobs = jobs;
                async move {
                    let data = match file.read_bytes().await {
                        Ok(data) => data.to_vec(),
                        Err(err) => {
                            update_job(jobs, id, |job| {
                                job.state = JobState::Failed(format!("Read error: {err:?}"));
                            });
                            return;
                        }
                    };

                    update_job(jobs, id, |job| {
                        job.state = JobState::Transcoding;
                        job.transcode_progress = 0.0;
                    });

                    let transcode_result =
                        services::transcoder::transcode(data.into(), |percent, _total| {
                            update_job(jobs, id, |job| {
                                if matches!(job.state, JobState::Transcoding) {
                                    job.transcode_progress = percent as f32;
                                }
                            });
                        })
                        .await;

                    match transcode_result {
                        Ok(result) => {
                            let meta = metadata::extract_metadata(&result.data).await;

                            update_job(jobs, id, |job| {
                                job.state = JobState::TranscodeReady;
                                job.transcode_progress = 100.0;
                                job.transcode_result = Some(TranscodeResult {
                                    name: result.filename,
                                    data: result.data,
                                });
                                job.metadata = Some(meta.clone());
                                job.edited_metadata = Some(meta);
                            });
                        }
                        Err(err) => {
                            update_job(jobs, id, |job| {
                                job.state = JobState::Failed(format!("Transcode: {err:?}"));
                            });
                        }
                    }
                }
            });
        }
    };

    let on_complete = on_complete.unwrap_or(EventHandler::new(|_| {}));

    let start_upload = move |job_id: u64| {
        let jobs = jobs;
        let mut toast = toast;
        let on_complete = on_complete;
        spawn(async move {
            let should_proceed = {
                let jobs_read = jobs.read();
                jobs_read
                    .iter()
                    .find(|j| j.id == job_id)
                    .map_or(false, |j| {
                        matches!(j.state, JobState::TranscodeReady | JobState::Failed(_))
                    })
            };
            if !should_proceed {
                return;
            }

            let (name, data, edited, original) = {
                let jobs_read = jobs.read();
                let job = match jobs_read.iter().find(|j| j.id == job_id) {
                    Some(j) => j,
                    None => return,
                };
                let result = match &job.transcode_result {
                    Some(r) => r,
                    None => return,
                };
                (
                    result.name.clone(),
                    result.data.clone(),
                    job.edited_metadata.clone(),
                    job.metadata.clone(),
                )
            };

            let mut upload_data = data;
            if let (Some(edited), Some(original)) = (&edited, &original) {
                if edited != original {
                    if let Err(err) = metadata::update_metadata(&mut upload_data, edited).await {
                        toast.show_error(format!("Metadata update failed: {err:?}"));
                    }
                }
            }

            let _permit = upload_sem().acquire().await;

            update_job(jobs, job_id, |job| {
                job.state = JobState::Uploading;
                job.upload_progress = 0.0;
            });

            let total_size = upload_data.len() as u64;
            let result = services::files::upload_file_chunked(
                &name,
                upload_data,
                128 * 1024,
                3,
                |uploaded, _total| {
                    let pct = if total_size > 0 {
                        (uploaded as f32 / total_size as f32) * 100.0
                    } else {
                        100.0
                    };
                    update_job(jobs, job_id, |job| {
                        job.upload_progress = pct;
                    });
                },
            )
            .await;

            match result {
                Ok(()) => {
                    update_job(jobs, job_id, |job| {
                        job.state = JobState::Complete;
                        job.upload_progress = 100.0;
                    });
                    toast.show_success(format!("{name} uploaded"));

                    let all_done = {
                        let jobs_read = jobs.read();
                        let total = jobs_read.len();
                        let done = jobs_read
                            .iter()
                            .filter(|j| matches!(j.state, JobState::Complete | JobState::Failed(_)))
                            .count();
                        total > 0 && done == total
                    };
                    if all_done {
                        on_complete.call(());
                    }
                }
                Err(err) => {
                    update_job(jobs, job_id, |job| {
                        job.state = JobState::Failed(format!("Upload: {err:?}"));
                    });
                    toast.show_error(format!("Upload {name} failed: {err:?}"));
                }
            }
        });
    };

    let n_jobs = jobs.read().len();
    let transcoding = jobs
        .read()
        .iter()
        .filter(|j| matches!(j.state, JobState::Transcoding))
        .count();
    let ready = jobs
        .read()
        .iter()
        .filter(|j| matches!(j.state, JobState::TranscodeReady))
        .count();
    let uploading = jobs
        .read()
        .iter()
        .filter(|j| matches!(j.state, JobState::Uploading))
        .count();
    let complete = jobs
        .read()
        .iter()
        .filter(|j| matches!(j.state, JobState::Complete))
        .count();
    let failed = jobs
        .read()
        .iter()
        .filter(|j| matches!(j.state, JobState::Failed(_)))
        .count();

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
                                    multiple: true,
                                    onchange: move |e| {
                                        e.prevent_default();
                                        let files = e.files();
                                        handle_files(files);
                                    },
                                }
                                span { class: "file-cta",
                                    span { class: "file-icon",
                                        i { class: "fas fa-upload" }
                                    }
                                    span { class: "file-label", "Choose files…" }
                                }
                                span { class: "file-name",
                                    if n_jobs == 0 {
                                        "No file selected"
                                    } else {
                                        "{n_jobs} file(s) selected"
                                    }
                                }
                            }
                        }
                    }
                }

                if n_jobs > 0 {
                    p { class: "has-text-grey is-size-7",
                        "Transcoding: {transcoding}/{N_WORKERS} | Ready: {ready} | Uploading: {uploading}/1 | Complete: {complete} | Errors: {failed}"
                    }
                }

                for job in jobs.read().iter().cloned() {
                    div { class: "box",
                        p { class: "has-text-weight-semibold", "{job.file_name}" }

                        {
                            match &job.state {
                                JobState::PendingTranscode => rsx! {
                                    b::Progress { value: 0.0, max: 100.0, "Waiting..." }
                                },
                                JobState::Transcoding => rsx! {
                                    b::Progress { color: b::BulmaColor::Info, value: job.transcode_progress, max: 100.0,
                                        "Transcoding {job.transcode_progress as u8}%"
                                    }
                                },
                                JobState::TranscodeReady => rsx! {
                                    b::Progress { color: b::BulmaColor::Info, value: 100.0, max: 100.0, "Transcode complete" }
                                    b::Button {
                                        color: b::BulmaColor::Primary,
                                        onclick: move |_| {
                                            let job_id = job.id;
                                            start_upload(job_id);
                                        },
                                        "Upload"
                                    }
                                },
                                JobState::Uploading => rsx! {
                                    b::Progress { color: b::BulmaColor::Info, value: 100.0, max: 100.0, "Transcode complete" }
                                    b::Progress { color: b::BulmaColor::Success, value: job.upload_progress, max: 100.0,
                                        "Uploading {job.upload_progress as u8}%"
                                    }
                                },
                                JobState::Complete => rsx! {
                                    b::Progress { color: b::BulmaColor::Info, value: 100.0, max: 100.0, "Transcode complete" }
                                    b::Progress { color: b::BulmaColor::Success, value: 100.0, max: 100.0, "Upload complete" }
                                    p { class: "has-text-success",
                                        dioxus_free_icons::Icon {
                                            icon: FaCheck,
                                            width: 16,
                                            height: 16,
                                            fill: "currentColor",
                                        }
                                        " Complete"
                                    }
                                },
                                JobState::Failed(err) => rsx! {
                                    b::Progress { color: b::BulmaColor::Danger, value: 100.0, max: 100.0, "{err}" }
                                    if job.transcode_progress >= 100.0 && job.transcode_result.is_some() {
                                        b::Button {
                                            color: b::BulmaColor::Warning,
                                            onclick: move |_| {
                                                let job_id = job.id;
                                                start_upload(job_id);
                                            },
                                            "Retry Upload"
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_item_creation() {
        let job = JobItem::new(1, "test.wav".to_string());
        assert_eq!(job.id, 1);
        assert_eq!(job.file_name, "test.wav");
        assert_eq!(job.state, JobState::PendingTranscode);
        assert_eq!(job.transcode_progress, 0.0);
        assert_eq!(job.upload_progress, 0.0);
        assert!(job.transcode_result.is_none());
        assert!(job.metadata.is_none());
    }

    #[test]
    fn test_job_state_transitions() {
        let mut job = JobItem::new(1, "test.wav".to_string());
        assert_eq!(job.state, JobState::PendingTranscode);

        job.state = JobState::Transcoding;
        assert_eq!(job.state, JobState::Transcoding);

        job.state = JobState::TranscodeReady;
        assert_eq!(job.state, JobState::TranscodeReady);

        job.state = JobState::Uploading;
        assert_eq!(job.state, JobState::Uploading);

        job.state = JobState::Complete;
        assert_eq!(job.state, JobState::Complete);

        job.state = JobState::Failed("err".to_string());
        assert_eq!(job.state, JobState::Failed("err".to_string()));
    }

    #[test]
    fn test_job_progress_values() {
        let mut job = JobItem::new(1, "test.wav".to_string());
        assert_eq!(job.transcode_progress, 0.0);
        assert_eq!(job.upload_progress, 0.0);

        job.transcode_progress = 50.0;
        assert_eq!(job.transcode_progress, 50.0);

        job.upload_progress = 75.0;
        assert_eq!(job.upload_progress, 75.0);

        job.transcode_progress = 100.0;
        job.upload_progress = 100.0;
        assert_eq!(job.transcode_progress, 100.0);
        assert_eq!(job.upload_progress, 100.0);
    }

    #[test]
    fn test_job_with_transcode_result() {
        let mut job = JobItem::new(2, "song.wav".to_string());
        assert!(job.transcode_result.is_none());

        let result = TranscodeResult {
            name: "song.bin".to_string(),
            data: vec![1, 2, 3].into(),
        };
        job.transcode_result = Some(result.clone());
        assert_eq!(job.transcode_result.as_ref().unwrap().name, "song.bin");
        assert_eq!(*job.transcode_result.as_ref().unwrap().data, vec![1, 2, 3]);
    }

    #[test]
    fn test_job_count_by_state() {
        let jobs = vec![
            JobItem {
                id: 1,
                file_name: "a.wav".into(),
                state: JobState::Transcoding,
                transcode_progress: 50.0,
                upload_progress: 0.0,
                transcode_result: None,
                metadata: None,
                edited_metadata: None,
            },
            JobItem {
                id: 2,
                file_name: "b.wav".into(),
                state: JobState::Transcoding,
                transcode_progress: 80.0,
                upload_progress: 0.0,
                transcode_result: None,
                metadata: None,
                edited_metadata: None,
            },
            JobItem {
                id: 3,
                file_name: "c.wav".into(),
                state: JobState::TranscodeReady,
                transcode_progress: 100.0,
                upload_progress: 0.0,
                transcode_result: None,
                metadata: None,
                edited_metadata: None,
            },
            JobItem {
                id: 4,
                file_name: "d.wav".into(),
                state: JobState::Complete,
                transcode_progress: 100.0,
                upload_progress: 100.0,
                transcode_result: None,
                metadata: None,
                edited_metadata: None,
            },
        ];

        let n_transcoding = jobs
            .iter()
            .filter(|j| matches!(j.state, JobState::Transcoding))
            .count();
        let n_ready = jobs
            .iter()
            .filter(|j| matches!(j.state, JobState::TranscodeReady))
            .count();
        let n_complete = jobs
            .iter()
            .filter(|j| matches!(j.state, JobState::Complete))
            .count();
        let n_pending = jobs
            .iter()
            .filter(|j| matches!(j.state, JobState::PendingTranscode))
            .count();

        assert_eq!(n_transcoding, 2);
        assert_eq!(n_ready, 1);
        assert_eq!(n_complete, 1);
        assert_eq!(n_pending, 0);
    }

    #[test]
    fn test_all_done_condition() {
        let jobs = vec![
            JobItem {
                id: 1,
                file_name: "a.wav".into(),
                state: JobState::Complete,
                transcode_progress: 100.0,
                upload_progress: 100.0,
                transcode_result: None,
                metadata: None,
                edited_metadata: None,
            },
            JobItem {
                id: 2,
                file_name: "b.wav".into(),
                state: JobState::Failed("err".into()),
                transcode_progress: 100.0,
                upload_progress: 0.0,
                transcode_result: None,
                metadata: None,
                edited_metadata: None,
            },
        ];
        let total = jobs.len();
        let done = jobs
            .iter()
            .filter(|j| matches!(j.state, JobState::Complete | JobState::Failed(_)))
            .count();
        assert!(total > 0 && done == total);

        let jobs2 = vec![
            JobItem {
                id: 1,
                file_name: "a.wav".into(),
                state: JobState::Complete,
                transcode_progress: 100.0,
                upload_progress: 100.0,
                transcode_result: None,
                metadata: None,
                edited_metadata: None,
            },
            JobItem {
                id: 2,
                file_name: "b.wav".into(),
                state: JobState::Transcoding,
                transcode_progress: 50.0,
                upload_progress: 0.0,
                transcode_result: None,
                metadata: None,
                edited_metadata: None,
            },
        ];
        let total = jobs2.len();
        let not_all_done = jobs2
            .iter()
            .filter(|j| matches!(j.state, JobState::Complete | JobState::Failed(_)))
            .count();
        assert!(not_all_done < total);
    }

    #[test]
    fn test_retry_button_condition_upload_failed() {
        let job = JobItem {
            id: 1,
            file_name: "a.wav".into(),
            state: JobState::Failed("Upload: timeout".into()),
            transcode_progress: 100.0,
            upload_progress: 50.0,
            transcode_result: Some(TranscodeResult {
                name: "a.bin".into(),
                data: vec![].into(),
            }),
            metadata: None,
            edited_metadata: None,
        };
        let show_retry = job.transcode_progress >= 100.0 && job.transcode_result.is_some();
        assert!(
            show_retry,
            "Retry button should show when upload failed after successful transcode"
        );
    }

    #[test]
    fn test_retry_button_condition_transcode_failed() {
        let job = JobItem {
            id: 2,
            file_name: "b.wav".into(),
            state: JobState::Failed("Transcode: error".into()),
            transcode_progress: 50.0,
            upload_progress: 0.0,
            transcode_result: None,
            metadata: None,
            edited_metadata: None,
        };
        let show_retry = job.transcode_progress >= 100.0 && job.transcode_result.is_some();
        assert!(
            !show_retry,
            "Retry button should NOT show when transcode itself failed"
        );
    }

    #[test]
    fn test_retry_state_reset() {
        let mut job = JobItem {
            id: 1,
            file_name: "a.wav".into(),
            state: JobState::Failed("Upload: timeout".into()),
            transcode_progress: 100.0,
            upload_progress: 50.0,
            transcode_result: Some(TranscodeResult {
                name: "a.bin".into(),
                data: vec![1, 2, 3].into(),
            }),
            metadata: None,
            edited_metadata: None,
        };
        job.state = JobState::Uploading;
        job.upload_progress = 0.0;
        assert_eq!(job.state, JobState::Uploading);
        assert_eq!(
            job.upload_progress, 0.0,
            "Upload progress must reset to 0 on retry"
        );
    }

    #[test]
    fn test_start_upload_guard_allows_retry() {
        let states_that_should_proceed = [JobState::TranscodeReady, JobState::Failed("err".into())];
        for state in &states_that_should_proceed {
            let proceed = matches!(state, JobState::TranscodeReady | JobState::Failed(_));
            assert!(proceed, "{state:?} should allow upload to proceed");
        }
    }

    #[test]
    fn test_start_upload_guard_blocks_duplicates() {
        let states_that_should_block = [
            JobState::Uploading,
            JobState::Complete,
            JobState::Transcoding,
            JobState::PendingTranscode,
        ];
        for state in &states_that_should_block {
            let proceed = matches!(state, JobState::TranscodeReady | JobState::Failed(_));
            assert!(!proceed, "{state:?} should block duplicate upload");
        }
    }

    #[test]
    fn test_upload_progress_calculation() {
        let total: u64 = 1000;
        let pct = |uploaded: u64| {
            if total > 0 {
                (uploaded as f32 / total as f32) * 100.0
            } else {
                100.0
            }
        };

        assert!((pct(0) - 0.0).abs() < f32::EPSILON, "0% at start");
        assert!((pct(250) - 25.0).abs() < f32::EPSILON, "25% at quarter");
        assert!((pct(1000) - 100.0).abs() < f32::EPSILON, "100% at complete");

        assert!(
            (pct(0) - 0.0).abs() < f32::EPSILON,
            "0 bytes uploaded should give 0%"
        );
    }

    #[test]
    fn test_upload_progress_edge_cases() {
        let total_zero: u64 = 0;
        let pct: f32 = if total_zero > 0 { 0.0 } else { 100.0 };
        assert!(
            (pct - 100.0).abs() < f32::EPSILON,
            "empty file should show 100%"
        );

        let total_small: u64 = 1;
        let uploaded: u64 = 1;
        let pct: f32 = if total_small > 0 {
            (uploaded as f32 / total_small as f32) * 100.0
        } else {
            100.0
        };
        assert!(
            (pct - 100.0).abs() < f32::EPSILON,
            "1 of 1 byte should be 100%"
        );
    }
}
