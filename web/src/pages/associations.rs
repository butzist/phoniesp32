use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::FaPlus;
use dioxus_router::Link;

use crate::Route;

#[component]
pub fn Associations() -> Element {
    rsx! {
        b::Section {
            b::Container {
                b::Columns {
                    b::Column {
                        b::Title { size: b::TitleSize::Is4, "Associations" }
                    }
                    b::Column { class: "has-text-right",
                        Link { to: Route::NewAssociation {},
                            b::Button { color: b::BulmaColor::Primary,
                                dioxus_free_icons::Icon {
                                    icon: FaPlus,
                                    width: 16,
                                    height: 16,
                                    fill: "currentColor",
                                }
                                " New Association"
                            }
                        }
                    }
                }
                crate::components::AssociationTable {}
            }
        }
    }
}

