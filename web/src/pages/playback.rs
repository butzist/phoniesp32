use dioxus::prelude::*;
use dioxus_bulma as b;

use crate::components::{CurrentPlaylist, CurrentSong};
use crate::services::playback::{self, get_status, StatusResponse};

#[component]
pub fn Playback() -> Element {
    let mut status = use_signal(|| None::<StatusResponse>);
    let mut last_status = use_signal(|| None::<StatusResponse>);

    use_future(move || async move {
        loop {
            match get_status().await {
                Ok(new_status) => {
                    status.set(Some(new_status.clone()));
                    last_status.set(Some(new_status));
                }
                Err(e) => {
                    eprintln!("Failed to get status: {:?}", e);
                    status.set(None);
                    last_status.set(None);
                }
            }
            async_std::task::sleep(std::time::Duration::from_millis(1000)).await;
        }
    });

    // Only update playlist when playlist name actually changes
    let mut current_playlist = use_signal(|| None);
    let playlist_name =
        use_memo(move || status().as_ref().and_then(|s| s.playlist_name.clone()));

    let playlist_resource = use_resource(move || async move {
        let playlist_name = playlist_name.read().clone();
        if let Some(_playlist_name) = playlist_name {
            playback::get_current_playlist().await.ok()
        } else {
            None
        }
    });

    // Update current_playlist when resource changes
    use_effect(move || {
        if let Some(playlist) = playlist_resource.read().clone().flatten() {
            current_playlist.set(Some(playlist));
        } else {
            current_playlist.set(None);
        }
    });

    rsx! {
        b::Section {
            b::Container {
                b::Columns {
                    b::Column {
                        b::Card {
                            b::CardHeader {
                                b::CardHeaderTitle {
                                    "Now Playing"
                                }
                            }
                            b::CardContent {
                                CurrentSong { status, current_playlist }
                            }
                        }
                    }
                    b::Column {
                        CurrentPlaylist { status, current_playlist }
                    }
                }

            }
        }
    }
}
