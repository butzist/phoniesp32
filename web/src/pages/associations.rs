use dioxus::prelude::*;
use dioxus_bulma as b;

#[component]
pub fn Associations() -> Element {
    rsx! {
        b::Section {
            b::Container {
                b::Title { size: b::TitleSize::Is4, "Associations" }
                crate::components::AssociationTable {}
            }
        }
    }
}

