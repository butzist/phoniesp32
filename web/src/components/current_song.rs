use dioxus::prelude::*;
use dioxus_bulma as b;

use crate::services::playback::{get_status, State, Status};

#[component]
pub fn CurrentSong() -> Element {
    let mut status = use_signal(|| Status {
        state: State::Stopped,
        position_seconds: None,
        metadata: None,
    });

    use_future(move || async move {
        loop {
            match get_status().await {
                Ok(new_status) => status.set(new_status),
                Err(e) => {
                    eprintln!("Failed to get status: {:?}", e);
                    status.set(Status {
                        state: State::Stopped,
                        position_seconds: None,
                        metadata: None,
                    });
                }
            }
            async_std::task::sleep(std::time::Duration::from_millis(2000)).await;
        }
    });

    let current_song = status
        .read()
        .metadata
        .as_ref()
        .map(|m| format!("{} - {}", m.artist, m.title))
        .unwrap_or_else(|| "No song playing".to_string());

    rsx! {
        b::Title { size: b::TitleSize::Is3, class: "has-text-centered", "{current_song}" }
    }
}