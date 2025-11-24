use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::FaUpload;

#[component]
pub fn Files() -> Element {
    let mut modal_open = use_signal(|| false);

    rsx! {
        b::Section {
            b::Container {
                b::Columns {
                    b::Column {
                        b::Title { size: b::TitleSize::Is4, "Files" }
                    }
                    b::Column { class: "has-text-right",
                        b::Button {
                            color: b::BulmaColor::Primary,
                            onclick: move |_| modal_open.set(true),
                            dioxus_free_icons::Icon {
                                icon: FaUpload,
                                width: 16,
                                height: 16,
                                fill: "currentColor",
                            }
                            " Upload"
                        }
                    }
                }
                crate::components::FileTable {}
            }
        }
        crate::components::Modal {
            active: *modal_open.read(),
            title: "Upload Audio File".to_string(),
            on_close: Some(EventHandler::new(move |_| modal_open.set(false))),
            crate::components::Upload { on_complete: Some(EventHandler::new(move |_| modal_open.set(false))) }
        }
    }
}
