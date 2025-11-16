use dioxus::prelude::*;
use dioxus_bulma as b;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: usize,
    pub message: String,
    pub color: b::BulmaColor,
    pub duration: Option<Duration>,
}

impl Toast {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            id: 0,
            message: message.into(),
            color: b::BulmaColor::Success,
            duration: Some(Duration::from_secs(3)),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            id: 0,
            message: message.into(),
            color: b::BulmaColor::Danger,
            duration: Some(Duration::from_secs(5)),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            id: 0,
            message: message.into(),
            color: b::BulmaColor::Warning,
            duration: Some(Duration::from_secs(4)),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self {
            id: 0,
            message: message.into(),
            color: b::BulmaColor::Info,
            duration: Some(Duration::from_secs(3)),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ToastManager {
    toasts: Signal<Vec<Toast>>,
    next_id: Signal<usize>,
}

#[allow(unused)]
impl ToastManager {
    pub fn new() -> Self {
        Self {
            toasts: Signal::new(Vec::new()),
            next_id: Signal::new(1),
        }
    }

    pub fn show(&mut self, toast: Toast) {
        let id = *self.next_id.read();
        *self.next_id.write() += 1;

        let mut toast_with_id = toast;
        toast_with_id.id = id;

        let duration = toast_with_id.duration;

        self.toasts.write().push(toast_with_id);

        if let Some(duration) = duration {
            let mut toasts_signal = self.toasts;
            spawn(async move {
                async_std::task::sleep(duration).await;
                toasts_signal.write().retain(|t| t.id != id);
            });
        }
    }

    pub fn show_success(&mut self, message: impl Into<String>) {
        self.show(Toast::success(message));
    }

    pub fn show_error(&mut self, message: impl Into<String>) {
        self.show(Toast::error(message));
    }

    pub fn show_warning(&mut self, message: impl Into<String>) {
        self.show(Toast::warning(message));
    }

    pub fn show_info(&mut self, message: impl Into<String>) {
        self.show(Toast::info(message));
    }

    pub fn dismiss(&mut self, id: usize) {
        self.toasts.write().retain(|t| t.id != id);
    }

    pub fn dismiss_all(&mut self) {
        self.toasts.set(Vec::new());
    }

    pub fn toasts(&self) -> Signal<Vec<Toast>> {
        self.toasts
    }
}

pub fn use_toast() -> ToastManager {
    use_context()
}

#[component]
pub fn ToastContainer() -> Element {
    let mut toast_manager = use_toast();
    let toasts = toast_manager.toasts();

    rsx! {
        div {
            class: "toast-container",
            style: "position: fixed; bottom: 20px; right: 20px; z-index: 9999; display: flex; flex-direction: column; gap: 10px;",
            {
                toasts
                    .read()
                    .iter()
                    .map(|toast_item| {
                        let toast_id = toast_item.id;
                        let toast_message = toast_item.message.clone();
                        let toast_color = toast_item.color;
                        rsx! {
                            b::Notification {
                                key: "{toast_id}",
                                color: toast_color,
                                onclose: move |_| {
                                    toast_manager.dismiss(toast_id);
                                },
                                "{toast_message}"
                            }
                        }
                    })
            }
        }
    }
}
