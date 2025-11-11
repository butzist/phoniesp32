use dioxus::prelude::*;
use dioxus_bulma as b;

use crate::components::ControlsButton;
use crate::services::playback::{
    next, previous, stop, toggle_pause, volume_down, volume_up, PlaybackState, StatusResponse,
};
use dioxus_free_icons::icons::fa_solid_icons::{
    FaCircleLeft, FaCirclePause, FaCirclePlay, FaCircleRight, FaCircleStop, FaVolumeHigh,
    FaVolumeLow,
};

#[component]
pub fn PlaybackControls(status: ReadSignal<Option<StatusResponse>>) -> Element {
    let current_status = status.read();
    let state = current_status
        .as_ref()
        .map(|s| &s.state)
        .unwrap_or(&PlaybackState::Stopped);
    let is_playing = matches!(state, PlaybackState::Playing);

    rsx! {
        b::Columns {
            multiline: true,
            centered: true,
            b::Column {
                size: b::ColumnSize::Half,
                b::Buttons {
                    ControlsButton {
                        icon: FaCircleLeft,
                        label: "Previous".to_string(),
                        onclick: |_| {
                            spawn(async move {
                                if let Err(e) = previous().await {
                                    error!("Failed to go to previous: {:?}", e);
                                }
                            });
                        },
                        disabled: *state == PlaybackState::Stopped,
                    }
                    if is_playing {
                        ControlsButton {
                            icon: FaCirclePause,
                            label: "Pause".to_string(),
                            onclick: move |_| {
                                spawn(async move {
                                    if let Err(e) = toggle_pause().await {
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
                                    if let Err(e) = toggle_pause().await {
                                        error!("Failed to unpause: {:?}", e);
                                    }
                                });
                            },
                            disabled: *state == PlaybackState::Stopped,
                        }
                    }
                    ControlsButton {
                        icon: FaCircleStop,
                        label: "Stop".to_string(),
                        onclick: |_| {
                            spawn(async move {
                                if let Err(e) = stop().await {
                                    error!("Failed to stop: {:?}", e);
                                }
                            });
                        },
                        disabled: *state == PlaybackState::Stopped,
                    }
                    ControlsButton {
                        icon: FaCircleRight,
                        label: "Next".to_string(),
                        onclick: |_| {
                            spawn(async move {
                                if let Err(e) = next().await {
                                    error!("Failed to go to next: {:?}", e);
                                }
                            });
                        },
                        disabled: *state == PlaybackState::Stopped,
                    }
                }
            }
            b::Column {
                size: b::ColumnSize::OneQuarter,
                b::Buttons {
                    ControlsButton {
                        icon: FaVolumeLow,
                        label: "Volume Down".to_string(),
                        onclick: |_| {
                            spawn(async move {
                                if let Err(e) = volume_down().await {
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
                                if let Err(e) = volume_up().await {
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

