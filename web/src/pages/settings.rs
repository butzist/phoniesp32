use crate::components::use_toast;
use crate::components::Notification;
use crate::services;
use dioxus::prelude::*;
use dioxus_bulma::{self as b, InputType};

#[component]
pub fn Settings() -> Element {
    let mut ssid = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut toast = use_toast();

    let set_config = {
        move |_| {
            let config = services::config::DeviceConfig {
                ssid: ssid.read().clone(),
                password: password.read().clone(),
            };

            spawn(async move {
                if let Err(_e) = services::config::put_config(&config).await {
                    toast.show_error("Failed to set WiFi credentials");
                } else {
                    toast.show_success("WiFi credentials set successfully");
                }
            });
        }
    };

    let delete_config = {
        move |_| {
            spawn(async move {
                if let Err(_e) = services::config::delete_config().await {
                    toast.show_error("Failed to delete WiFi credentials");
                } else {
                    ssid.set(String::new());
                    password.set(String::new());
                    toast.show_success("WiFi credentials deleted successfully");
                }
            });
        }
    };

    rsx! {
        b::Section {
            b::Container {
                Notification {
                    color: b::BulmaColor::Info,
                    b::Title { size: b::TitleSize::Is5, "Access Point Mode" }
                    p {
                        "When no WiFi credentials are configured, the device will automatically start in Access Point (AP) mode. Connect to the network with SSID "
                        strong { "phoniesp32" }
                        " and password "
                        strong { "12345678" }
                        " to access the device."
                    }
                }

                b::Field {
                    b::Label { "WiFi SSID" }
                    b::Control {
                        b::Input {
                            input_type: InputType::Text,
                            placeholder: "Enter WiFi SSID",
                            value: "{ssid.read()}",
                            oninput: move |e: Event<FormData>| ssid.set(e.value()),
                        }
                    }
                }

                b::Field {
                    b::Label { "WiFi Password" }
                    b::Control {
                        b::Input {
                            input_type: InputType::Password,
                            placeholder: "Enter WiFi Password",
                            value: "{password.read()}",
                            oninput: move |e: Event<FormData>| password.set(e.value()),
                        }
                    }
                }

                b::Field { grouped: true,
                    b::Control {
                        b::Button {
                            color: b::BulmaColor::Primary,
                            disabled: ssid.read().is_empty() || password.read().len() < 8,
                            onclick: set_config,
                            "Set WiFi Credentials"
                        }
                    }
                    b::Control {
                        b::Button {
                            color: b::BulmaColor::Danger,
                            onclick: delete_config,
                            "Delete WiFi Credentials"
                        }
                    }
                }
            }
        }
    }
}
