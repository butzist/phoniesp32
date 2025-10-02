pub use dioxus::prelude::*;
use dioxus_free_icons::{Icon, IconShape};

#[component]
pub fn ControlsButton<T: IconShape + PartialEq + Clone + 'static>(
    icon: T,
    label: String,
) -> Element {
    rsx! {
        button { class: "btn secondary",
            Icon {
                width: 24,
                height: 24,
                fill: "black",
                title: label,
                icon: icon,
            },
        }
    }
}
