use dioxus::prelude::*;
use dioxus_bulma as b;

use crate::components::PlaybackControls;
use crate::services::playback::{CurrentPlaylistResponse, PlaybackState, StatusResponse};

#[component]
pub fn CurrentSong(
    status: ReadSignal<Option<StatusResponse>>,
    current_playlist: ReadSignal<Option<CurrentPlaylistResponse>>,
) -> Element {
    let current_song = use_memo(move || {
        let result: Option<String> = try {
            let index = status().as_ref()?.index_in_playlist;
            let playlist = current_playlist()?;
            let current_file = playlist.files.get(index)?;

            current_file
                .metadata
                .as_ref()
                .map(|m| format!("{} - {}", m.artist, m.title))?
        };

        result.unwrap_or_else(|| "No song playing".to_string())
    });

    let (status_color_class, status_icon_name) = match status()
        .as_ref()
        .map(|s| &s.state)
        .unwrap_or(&PlaybackState::Stopped)
    {
        PlaybackState::Playing => ("success", "▶"),
        PlaybackState::Paused => ("warning", "⏸"),
        PlaybackState::Stopped => ("primary", "⏹"),
    };

    let position_display = use_memo(move || {
        status()
            .as_ref()
            .map(|s| {
                if s.position_seconds > 0 {
                    format!("{}:{:02}", s.position_seconds / 60, s.position_seconds % 60)
                } else {
                    "--:--".to_string()
                }
            })
            .unwrap_or_else(|| "--:--".to_string())
    });

    rsx! {
        div {
            b::Columns {
                b::Column {
                    b::Title { size: b::TitleSize::Is4, "{current_song}" }
                }
                b::Column { class: "has-text-centered",
                    div { class: "has-text-centered",
                        div {
                            class: "icon is-large has-text-{status_color_class}",
                            style: "font-size: 3rem;",
                            "{status_icon_name}"
                        }
                        b::Title { size: b::TitleSize::Is5, class: "mt-2", "{position_display}" }
                    }
                }
            }
            div { class: "mt-4",
                PlaybackControls { status }
            }
        }
    }
}
