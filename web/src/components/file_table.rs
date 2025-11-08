use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::FaPlay;

use crate::components::ControlsButton;
use crate::services;

#[component]
pub fn FileTable() -> Element {
    let files_resource = use_resource(|| async {
        match services::files::list_files().await {
            Ok(files) => files,
            Err(e) => {
                eprintln!("Failed to list files: {:?}", e);
                vec![]
            }
        }
    });

    rsx! {
        b::Table {
            fullwidth: true,
            thead {
                tr {
                    th { "Play" }
                    th { "Artist" }
                    th { "Title" }
                    th { "Album" }
                    th { "Duration" }
                }
            }
            tbody {
                if let Some(vec) = files_resource.read().as_ref().map(|r| (**r).to_vec()) {
                    for entry in vec {
                        tr {
                            td {
                                ControlsButton {
                                    icon: FaPlay,
                                    label: "Play".to_string(),
                                    size: Some(b::BulmaSize::Small),
                                    onclick: Some(EventHandler::new(move |_| {
                                        let name = entry.name.clone();
                                        spawn(async move {
                                            if let Err(e) = services::playback::play_file(&name).await {
                                                eprintln!("Failed to play file: {:?}", e);
                                            }
                                        });
                                    })),
                                }
                            }
                            td { "{entry.metadata.artist}" }
                            td { "{entry.metadata.title}" }
                            td { "{entry.metadata.album}" }
                            td {
                                {
                                    let duration = entry.metadata.duration;
                                    let minutes = duration / 60;
                                    let seconds = duration % 60;
                                    format!("{}:{:02}", minutes, seconds)
                                }
                            }
                        }
                    }
                 } else {
                     tr {
                         td { colspan: 5, "Loading files..." }
                     }
                 }
            }
        }
    }
}