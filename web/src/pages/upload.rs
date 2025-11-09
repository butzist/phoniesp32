use dioxus::prelude::*;
use dioxus_bulma as b;

#[component]
pub fn UploadPage() -> Element {
    rsx! {
        b::Section {
            b::Container {
                b::Title { size: b::TitleSize::Is4, "Upload Audio File" }
                crate::components::Upload {}
            }
        }
    }
}