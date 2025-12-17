use dioxus::prelude::*;
use dioxus_bulma as b;

#[component]
pub fn Notification(mut props: b::NotificationProps) -> Element {
    if let Some(color) = props.color {
        let gradient_class = match color {
            b::BulmaColor::Primary => "is-notification-gradient-primary",
            b::BulmaColor::Link => "is-notification-gradient-link",
            b::BulmaColor::Info => "is-notification-gradient-info",
            b::BulmaColor::Success => "is-notification-gradient-success",
            b::BulmaColor::Warning => "is-notification-gradient-warning",
            b::BulmaColor::Danger => "is-notification-gradient-danger",
            _ => "is-notification-gradient-primary",
        };

        props.class = Some(if let Some(additional_class) = props.class {
            format!("notification {} {}", gradient_class, additional_class)
        } else {
            format!("notification {}", gradient_class)
        });
    };

    b::Notification(props)
}
