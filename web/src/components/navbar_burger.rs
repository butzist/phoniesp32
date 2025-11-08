use dioxus::prelude::*;

#[component]
pub fn NavbarBurger(is_active: Signal<bool>) -> Element {
    rsx! {
        button { class: "navbar-burger has-text-primary", "aria-expanded": "{is_active()}", "aria-label": "menu", onclick: move |_| is_active.set(!is_active()),
            span { "aria-hidden": "true" }
            span { "aria-hidden": "true" }
            span { "aria-hidden": "true" }
            span { "aria-hidden": "true" }
        }
    }
}