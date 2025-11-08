use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::{FaLink, FaPlay};
use dioxus_free_icons::Icon;
use dioxus_router::Link;

use crate::components::NavbarBurger;
use crate::Route;

#[component]
pub fn Layout(children: Element) -> Element {
    let is_active = use_signal(|| false);
    rsx! {
        b::Navbar { class: if is_active() { "is-active" } else { "" },
            b::NavbarBrand {
                b::NavbarItem {
                    b::Title { size: b::TitleSize::Is4, "Phoniesp32" }
                }
                NavbarBurger { is_active }
            }
            b::NavbarMenu { class: if is_active() { "is-active" } else { "" },
                b::NavbarEnd {
                    b::NavbarItem {
                        Link { to: Route::Playback {},
                            span { class: "has-text-primary",
                                Icon { icon: FaPlay, width: 16, height: 16, fill: "currentColor" }
                                " Playback"
                            }
                        }
                    }
                    b::NavbarItem {
                        Link { to: Route::Associations {},
                            span { class: "has-text-primary",
                                Icon { icon: FaLink, width: 16, height: 16, fill: "currentColor" }
                                " Associations"
                            }
                        }
                    }
                }
            }
        }
        Outlet::<Route> {}
    }
}
