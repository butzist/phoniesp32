use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::FaLink;

use crate::components::ControlsButton;
use crate::services;

#[component]
pub fn NewAssociation() -> Element {
    let mut last_fob = use_signal(|| None::<String>);

    use_future(move || async move {
        loop {
            match services::fob::get_last_fob().await {
                Ok(fob) => last_fob.set(fob),
                Err(e) => {
                    eprintln!("Failed to get last fob: {:?}", e);
                    last_fob.set(None);
                }
            }
            async_std::task::sleep(std::time::Duration::from_millis(2000)).await;
        }
    });

    let files_resource = use_resource(|| async {
        match services::files::list_files().await {
            Ok(files) => files,
            Err(e) => {
                eprintln!("Failed to list files: {:?}", e);
                vec![]
            }
        }
    });

    rsx! {
        b::Section {
            b::Container {
                b::Title { size: b::TitleSize::Is4, "New Association" }
                b::Notification {
                    color: if last_fob().is_some() { b::BulmaColor::Info } else { b::BulmaColor::Warning },
                    if let Some(fob) = last_fob() {
                        span { "Select a file to associate with last scanned FOB: " b { "{fob}" } }
                    } else {
                        "No FOB scanned recently. Please scan a FOB first."
                    }
                }
                if last_fob().is_some() {
                    b::Table {
                        fullwidth: true,
                        thead {
                            tr {
                                th { "Associate" }
                                th { "Artist" }
                                th { "Title" }
                                th { "Album" }
                                th { "Duration" }
                            }
                        }
                        tbody {
                            if let Some(vec) = files_resource.read().as_ref().map(|r| (**r).to_vec()) {
                                for entry in vec {
                                    tr {
                                        td {
                                            ControlsButton {
                                                icon: FaLink,
                                                label: "Associate".to_string(),
                                                size: Some(b::BulmaSize::Small),
                                                onclick: Some(EventHandler::new(move |_| {
                                                    let file_name = entry.name.clone();
                                                    let fob = last_fob().unwrap().clone();
                                                    spawn(async move {
                                                        if let Err(e) = services::fob::associate_fob(&fob, &file_name).await {
                                                            eprintln!("Failed to associate: {:?}", e);
                                                        }
                                                    });
                                                })),
                                            }
                                        }
                                        td { "{entry.metadata.artist}" }
                                        td { "{entry.metadata.title}" }
                                        td { "{entry.metadata.album}" }
                                        td {
                                            {
                                                let duration = entry.metadata.duration;
                                                let minutes = duration / 60;
                                                let seconds = duration % 60;
                                                format!("{}:{:02}", minutes, seconds)
                                            }
                                        }
                                    }
                                }
                            } else {
                                tr {
                                    td { colspan: 5, "Loading files..." }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

