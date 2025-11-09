use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::{FaFile, FaLink, FaPlay, FaUpload, FaWrench};
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
                        Link { to: Route::Files {},
                            span { class: "has-text-primary",
                                Icon { icon: FaFile, width: 16, height: 16, fill: "currentColor" }
                                " Files"
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
                    b::NavbarItem { class: "has-dropdown is-hoverable",
                        a { class: "navbar-link has-text-primary",
                            "Create"
                        }
                        div { class: "navbar-dropdown",
                            b::NavbarItem {
                                Link { to: Route::UploadPage {},
                                    span { class: "has-text-primary",
                                        Icon { icon: FaUpload, width: 16, height: 16, fill: "currentColor" }
                                        " File"
                                    }
                                }
                            }
                            b::NavbarItem {
                                Link { to: Route::NewAssociation {},
                                    span { class: "has-text-primary",
                                        Icon { icon: FaLink, width: 16, height: 16, fill: "currentColor" }
                                        " Association"
                                    }
                                }
                            }
                        }
                    }
                    b::NavbarItem {
                        Link { to: Route::Settings {},
                            span { class: "has-text-primary",
                                Icon { icon: FaWrench, width: 16, height: 16, fill: "currentColor" }
                                " Settings"
                            }
                        }
                    }
                }
            }
        }
        Outlet::<Route> {}
    }
}
