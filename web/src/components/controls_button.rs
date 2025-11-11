use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::{Icon, IconShape};

#[component]
pub fn ControlsButton<T: IconShape + PartialEq + Clone + 'static>(
    icon: T,
    label: String,
    onclick: Option<EventHandler<MouseEvent>>,
    size: Option<b::BulmaSize>,
    #[props(default)] disabled: bool,
) -> Element {
    let icon_size = match size {
        Some(b::BulmaSize::Small) => 12,
        _ => 24,
    };
    rsx! {
        b::Button {
            color: b::BulmaColor::Primary,
            size: size,
            onclick: onclick,
            disabled: disabled,
            Icon {
                width: icon_size,
                height: icon_size,
                fill: "currentColor",
                title: label,
                icon: icon,
            }
        }
    }
}
