use dioxus::prelude::*;
use dioxus_bulma as b;

#[component]
pub fn Associations() -> Element {
    rsx! {
        b::Section {
            b::Container {
                b::Subtitle { size: b::TitleSize::Is3, "Associations" }
                crate::components::AssociationTable {}
            }
        }
    }
}

