use dioxus::prelude::*;

use crate::components::ControlsButton;
use dioxus_free_icons::icons::fa_regular_icons::*;

#[component]
pub fn Controls() -> Element {
    rsx! {
        div {
            display: "flex",
            justify_content: "center",
            align_items: "center",
            gap: "10px",

            ControlsButton { icon: FaCircleLeft, label: "Previous" }
            ControlsButton { icon: FaCirclePlay, label: "Play" }
            ControlsButton { icon: FaCirclePause, label: "Pause" }
            ControlsButton { icon: FaCircleStop, label: "Stop" }
            ControlsButton { icon: FaCircleRight, label: "Next" }
        }
    }
}
