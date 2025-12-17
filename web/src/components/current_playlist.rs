use crate::components::Notification;
use crate::services::playback;
use dioxus::prelude::*;
use dioxus_bulma as b;

#[component]
pub fn CurrentPlaylist(
    status: ReadSignal<Option<playback::StatusResponse>>,
    current_playlist: ReadSignal<Option<playback::CurrentPlaylistResponse>>,
) -> Element {
    let playlist = current_playlist();
    let current_file_index = status().as_ref().map(|c| c.index_in_playlist);

    rsx! {
        b::Card {
            class: "has-background-primary-soft is-card-hover-lift",
            b::CardHeader {
                class: "is-card-header-gradient",
                b::CardHeaderTitle { "Current Playlist" }
            }
            b::CardContent {
                if let Some(playlist) = playlist.as_ref() {
                    div { style: "margin-bottom: 1rem;",
                        b::Title { size: b::TitleSize::Is6, "{playlist.playlist_name}" }
                        b::Tags {
                            b::Tag { color: b::BulmaColor::Info, "{playlist.files.len()} files" }
                        }
                    }

                    b::Table { fullwidth: true,
                        thead {
                            tr {
                                th { "Song" }
                                th { "Duration" }
                                th { "Status" }
                            }
                        }
                        tbody {
                            for (index , file) in playlist.files.iter().enumerate() {
                                tr { key: "{index}",
                                    td {
                                        if let Some(metadata) = &file.metadata {
                                            div {
                                                div { style: "font-weight: 500;",
                                                    "{metadata.artist} - {metadata.title}"
                                                }
                                                if !metadata.album.is_empty() {
                                                    div { class: "has-text-grey is-size-7",
                                                        "{metadata.album}"
                                                    }
                                                }
                                            }
                                        } else {
                                            div { style: "font-weight: 500;", "{file.file}" }
                                        }
                                    }
                                    td {
                                        if let Some(metadata) = &file.metadata {
                                            "{metadata.duration / 60}:{metadata.duration % 60:02}"
                                        } else {
                                            "Unknown"
                                        }
                                    }
                                    td {
                                        if Some(index) == current_file_index {
                                            b::Tag { color: b::BulmaColor::Success, "Playing" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    Notification {
                        color: b::BulmaColor::Info,
                        "No playlist currently loaded"
                    }
                }
            }
        }
    }
}
