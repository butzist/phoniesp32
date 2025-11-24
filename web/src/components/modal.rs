use dioxus::prelude::*;

#[component]
pub fn Modal(
    active: bool,
    title: String,
    on_close: Option<EventHandler<()>>,
    children: Element,
) -> Element {
    rsx! {
        div { class: if active { "modal is-active" } else { "modal" },
            div {
                class: "modal-background",
                onclick: move |_| {
                    if let Some(on_close) = on_close.as_ref() {
                        on_close.call(());
                    }
                },
            }
            div { class: "modal-card",
                header { class: "modal-card-head",
                    p { class: "modal-card-title", "{title}" }
                    button {
                        class: "delete",
                        onclick: move |_| {
                            if let Some(on_close) = on_close.as_ref() {
                                on_close.call(());
                            }
                        },
                    }
                }
                section { class: "modal-card-body", {children} }
            }
        }
    }
}
