use dioxus::prelude::*;
use dioxus_bulma as b;

use crate::components::ControlsButton;
use crate::services::playback;
use dioxus_free_icons::icons::fa_solid_icons::{FaVolumeHigh, FaVolumeLow, FaCirclePlay, FaCirclePause, FaCircleStop, FaCircleLeft, FaCircleRight};

#[component]
pub fn Controls(status: ReadOnlySignal<Option<playback::StatusResponse>>) -> Element {
    let current_status = status.read();
    let state = current_status.as_ref().map(|s| &s.state).unwrap_or(&playback::PlaybackState::Stopped);
    
    let is_playing = matches!(state, playback::PlaybackState::Playing);

    rsx! {
        div { style: "max-width: 600px; margin: 0 auto;",
            b::Columns { multiline: true, centered: true,
                b::Column { size: b::ColumnSize::Half,
                    b::Buttons {
                        ControlsButton {
                            icon: FaCircleLeft,
                            label: "Previous".to_string(),
                            onclick: |_| {
                                spawn(async move {
                                    if let Err(e) = playback::previous().await {
                                        error!("Failed to go to previous: {:?}", e);
                                    }
                                });
                            },
                            disabled: *state == playback::PlaybackState::Stopped,
                        }
                        if is_playing {
                            ControlsButton {
                                icon: FaCirclePause,
                                label: "Pause".to_string(),
                                onclick: move |_| {
                                    spawn(async move {
                                        if let Err(e) = playback::pause().await {
                                            error!("Failed to pause: {:?}", e);
                                        }
                                    });
                                },
                            }
                        } else {
                            ControlsButton {
                                icon: FaCirclePlay,
                                label: "Play".to_string(),
                                onclick: move |_| {
                                    spawn(async move {
                                        if let Err(e) = playback::play().await {
                                            error!("Failed to play: {:?}", e);
                                        }
                                    });
                                },
                            }
                        }
                        ControlsButton {
                            icon: FaCircleStop,
                            label: "Stop".to_string(),
                            onclick: |_| {
                                spawn(async move {
                                    if let Err(e) = playback::stop().await {
                                        error!("Failed to stop: {:?}", e);
                                    }
                                });
                            },
                            disabled: *state == playback::PlaybackState::Stopped,
                        }
                        ControlsButton {
                            icon: FaCircleRight,
                            label: "Next".to_string(),
                            onclick: |_| {
                                spawn(async move {
                                    if let Err(e) = playback::next().await {
                                        error!("Failed to go to next: {:?}", e);
                                    }
                                });
                            },
                            disabled: *state == playback::PlaybackState::Stopped,
                        }
                    }
                }
                b::Column { size: b::ColumnSize::OneQuarter,
                    b::Buttons {
                        ControlsButton {
                            icon: FaVolumeLow,
                            label: "Volume Down".to_string(),
                            onclick: |_| {
                                spawn(async move {
                                    if let Err(e) = playback::volume_down().await {
                                        error!("Failed to volume down: {:?}", e);
                                    }
                                });
                            },
                        }
                        ControlsButton {
                            icon: FaVolumeHigh,
                            label: "Volume Up".to_string(),
                            onclick: |_| {
                                spawn(async move {
                                    if let Err(e) = playback::volume_up().await {
                                        error!("Failed to volume up: {:?}", e);
                                    }
                                });
                            },
                        }
                    }
                }
            }
        }
    }
}