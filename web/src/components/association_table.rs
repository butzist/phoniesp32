use crate::components::ControlsButton;
use crate::services;
use crate::services::fob::Association;
use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::FaPlay;

#[derive(Clone, PartialEq)]
struct AssociationInfo {
    fob: String,
    count: usize,
    duration_str: String,
}

#[component]
pub fn AssociationTable() -> Element {
    let associations_resource =
        use_resource(|| async { services::fob::list_associations().await.unwrap_or_default() });

    let playlist_infos = use_memo(move || {
        if let Some(associations) = associations_resource.read().as_ref() {
            let mut infos = vec![];
            for Association { fob, files } in associations {
                let count = files.len();
                let total_duration: u32 = files.iter().map(|f| f.metadata.duration).sum();
                let duration_str = format!("{}:{:02}", total_duration / 60, total_duration % 60);
                infos.push(AssociationInfo {
                    fob: fob.to_string(),
                    count,
                    duration_str,
                });
            }
            Some(infos)
        } else {
            None
        }
    });

    rsx! {
        if let Some(infos) = playlist_infos() {
            b::Table { fullwidth: true,
                thead {
                    tr {
                        th { "Play" }
                        th { "Fob ID" }
                        th { "Songs" }
                        th { "Total Duration" }
                    }
                }
                tbody {
                    {
                        infos
                            .clone()
                            .into_iter()
                            .map(|info| {
                                let fob = info.fob.clone();
                                rsx! {
                                    tr {
                                        td {
                                            ControlsButton {
                                                icon: FaPlay,
                                                label: "Play Association".to_string(),
                                                size: Some(b::BulmaSize::Small),
                                                onclick: Some(
                                                    EventHandler::new(move |_| {
                                                        let value = fob.clone();
                                                        spawn(async move {
                                                            if let Err(e) = services::playback::play_playlist_ref(&value).await {
                                                                eprintln!("Failed to play playlist: {:?}", e);
                                                            }
                                                        });
                                                    }),
                                                ),
                                            }
                                        }
                                        td { "{info.fob}" }
                                        td { "{info.count}" }
                                        td { "{info.duration_str}" }
                                    }
                                }
                            })
                    }
                }
            }
        } else {
            "Loading..."
        }
    }
}
