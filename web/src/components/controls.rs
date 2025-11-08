use dioxus::prelude::*;
use dioxus_bulma as b;

use crate::components::ControlsButton;
use crate::services::playback;
use dioxus_free_icons::icons::fa_regular_icons::*;
use dioxus_free_icons::icons::fa_solid_icons::{FaVolumeHigh, FaVolumeLow};

#[component]
pub fn Controls() -> Element {
    rsx! {
        div { style: "max-width: 600px; margin: 0 auto;",
            b::Columns {
                multiline: true,
                centered: true,
                b::Column {
                    size: b::ColumnSize::Half,
                    b::Buttons {
                        ControlsButton {
                            icon: FaCircleLeft,
                            label: "Previous".to_string(),
                            onclick: |_| {},
                        }
                        ControlsButton {
                            icon: FaCirclePause,
                            label: "Pause".to_string(),
                            onclick: |_| {
                                spawn(async move {
                                    if let Err(e) = playback::pause().await {
                                        error!("Failed to pause: {:?}", e);
                                    }
                                });
                            },
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
                        }
                        ControlsButton {
                            icon: FaCircleRight,
                            label: "Next".to_string(),
                            onclick: |_| {},
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
