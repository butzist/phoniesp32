use std::sync::Arc;

use crate::services::transcoder;

use dioxus::{
    html::{FileEngine, HasFileData},
    prelude::*,
    web::WebEventExt as _,
};
use wasm_bindgen::JsCast;

#[component]
pub fn Upload() -> Element {
    let mut input_element: Signal<Option<web_sys::HtmlInputElement>> = use_signal(|| None);

    let mut progress = use_signal(|| 0);
    let mut conversion_running = use_signal(|| false);

    let mut file_name = use_signal(|| None);
    let mut file_data: Signal<Option<Box<[u8]>>> = use_signal(|| None);

    let upload_ready = use_memo(move || file_name().is_some() && file_data().is_some());

    let process_files = move |fileengine: Arc<dyn FileEngine>| async move {
        let Some(name) = fileengine.files().first().cloned() else {
            return;
        };
        conversion_running.set(true);

        file_name.set(Some(name.to_string()));

        let data = fileengine
            .read_file(&name)
            .await
            .expect("failed reading file");
        let progress = move |percent: usize, _total: usize| {
            if percent != progress() {
                progress.set(percent);
            }
        };
        let output = transcoder::transcode(data.into(), progress)
            .await
            .expect("failed transcoding");
        file_data.set(Some(output));

        conversion_running.set(false);
    };

    rsx! {
        div { class: "fileinput",
            div { class: "dropzone",
                id: "dropzone",
                hidden: upload_ready() || conversion_running(),

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
        div { class: "progress", hidden: !conversion_running(),
            div { class: "bar",
                width: "{progress}%",
            }
        }
        button { class: "btn primary",
            disabled: !upload_ready(),

            onclick: move |_| { web_sys::console::log_1(&format!("data len: {}", file_data().map(|samples| samples.len()).unwrap_or_default()).into()); },

            "Upload",
        }
    }
}
