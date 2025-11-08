use dioxus::prelude::*;
use dioxus_bulma as b;

#[component]
pub fn Playback() -> Element {
    rsx! {
        b::Section {
            b::Container {
                crate::components::CurrentSong {}
                crate::components::Controls {}
                b::Subtitle { size: b::TitleSize::Is3, "Files" }
                crate::components::FileTable {}
            }
        }
    }
}

