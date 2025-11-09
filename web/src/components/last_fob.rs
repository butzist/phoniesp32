use dioxus::prelude::*;
use dioxus_bulma as b;

use crate::services::fob::get_last_fob;

#[component]
pub fn LastFob() -> Element {
    let mut last_fob = use_signal(|| None::<String>);

    use_future(move || async move {
        loop {
            match get_last_fob().await {
                Ok(fob) => last_fob.set(fob),
                Err(e) => {
                    eprintln!("Failed to get last fob: {:?}", e);
                    last_fob.set(None);
                }
            }
            async_std::task::sleep(std::time::Duration::from_millis(2000)).await;
        }
    });

    let fob_text = last_fob().as_ref().map(|f| format!("Last scanned FOB: {}", f)).unwrap_or_else(|| "No FOB scanned recently".to_string());

    rsx! {
        b::Card {
            b::CardContent {
                b::Title { size: b::TitleSize::Is4, "{fob_text}" }
            }
        }
    }
}