use dioxus::prelude::*;

pub(crate) mod components;
pub(crate) mod services;

use components::{Controls, Upload};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    Home {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/styles.scss");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

/// Home page
#[component]
fn Home() -> Element {
    rsx! {
        h1 { "PhoniESP32" }

        div {
            display: "flex",
            justify_content: "center",
            align_items: "center",
            flex_direction: "column",
            gap: "20px",

            div { class: "card",
                width: "600px",

                div { class: "header", "Controls" }
                div { class: "content",
                    div {
                        display: "flex",
                        justify_content: "center",
                        align_items: "center",
                        flex_direction: "column",
                        gap: "20px",

                        div { "Currently playing: NOTHING" }
                        Controls {}
                    }
                }
            }
            div { class: "card",
                width: "600px",

                div { class: "header", "Upload & Convert" }
                div { class: "content",
                    Upload {}
                }
            }
        }
    }
}
