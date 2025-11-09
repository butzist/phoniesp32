#![feature(try_blocks)]
use dioxus::prelude::*;
use dioxus_bulma as b;
pub(crate) mod components;
pub(crate) mod layouts;
pub(crate) mod metadata;
pub(crate) mod pages;
pub(crate) mod services;

use layouts::Layout;
use pages::{Associations, Files, NewAssociation, Playback, Settings, UploadPage};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Layout)]
    #[route("/")]
    Playback {},

    #[route("/associations")]
    Associations {},

    #[route("/files")]
    Files {},

    #[route("/upload")]
    UploadPage {},

    #[route("/new-association")]
    NewAssociation {},

    #[route("/settings")]
    Settings {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        b::BulmaProvider { theme: b::BulmaTheme::Auto, load_bulma_css: true,
            Router::<Route> {}
        }
    }
}
