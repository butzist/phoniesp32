use dioxus::prelude::*;
use dioxus_bulma as b;
use dioxus_free_icons::icons::fa_solid_icons::{FaGripVertical, FaMinus, FaPlus};

use crate::components::use_toast;
use crate::components::ControlsButton;
use crate::components::Notification;
use crate::services;
use crate::services::utils::FileEntry;

fn get_file_info(
    files_resource: &Resource<Vec<FileEntry>>,
    file_name: &str,
    field: &str,
) -> String {
    if let Some(vec) = files_resource.read().as_ref().map(|r| (**r).to_vec()) {
        if let Some(entry) = vec.iter().find(|e: &&FileEntry| e.name == file_name) {
            match field {
                "artist" => entry.metadata.artist.clone(),
                "title" => entry.metadata.title.clone(),
                "album" => entry.metadata.album.clone(),
                "duration" => entry.metadata.duration.to_string(),
                _ => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        }
    } else {
        "Unknown".to_string()
    }
}

#[component]
pub fn NewAssociation() -> Element {
    let mut last_fob = use_signal(|| None);
    let mut playlist = use_signal(Vec::<String>::new);
    let mut dragged_index = use_signal(|| None::<usize>);
    let mut drop_target_index = use_signal(|| None::<usize>);
    let save_status = use_signal(|| None::<String>);
    let save_status_type = use_signal(|| b::BulmaColor::Success);
    let mut toast = use_toast();

    use_future({
        move || async move {
            loop {
                match services::fob::get_last_fob().await {
                    Ok(fob) => last_fob.set(fob),
                    Err(e) => {
                        toast.show_error(format!("Failed to get last FOB: {:?}", e));
                        last_fob.set(None);
                    }
                }
                async_std::task::sleep(std::time::Duration::from_millis(1000)).await;
            }
        }
    });

    let files_resource = use_resource(move || async move {
        match services::files::list_files().await {
            Ok(files) => files,
            Err(e) => {
                toast.show_error(format!("Failed to list files: {:?}", e));
                Vec::new()
            }
        }
    });

    rsx! {
        b::Section {
            b::Container {
                b::Title { size: b::TitleSize::Is4, "New Association" }
                Notification { 
                    color: if last_fob().is_some() { b::BulmaColor::Info } else { b::BulmaColor::Warning },
                    if let Some(fob) = last_fob() {
                        span {
                            "Create a playlist to associate with last scanned FOB: "
                            b { "{fob}" }
                        }
                    } else {
                        "No FOB scanned recently. Please scan a FOB first."
                    }
                }
                if last_fob().is_some() {
                    b::Columns { multiline: true,
                        b::Column { size: b::ColumnSize::Half,
                            b::Card {
                                class: "has-background-primary-soft is-card-hover-lift",
                                b::CardHeader {
                                    class: "is-card-header-gradient",
                                    b::CardHeaderTitle { "Playlist" }
                                }
                                b::CardContent {
                                    if playlist().is_empty() {
                                        div { class: "box has-text-grey has-border",
                                            "No items in playlist. Add files from the right."
                                        }
                                    } else {
                                        b::Table { fullwidth: true,
                                            thead {
                                                tr {
                                                    th { "" }
                                                    th { "Artist" }
                                                    th { "Title" }
                                                    th { "Duration" }
                                                    th { "" }
                                                }
                                            }
                                            tbody {
                                                for (index , file_name) in playlist().iter().enumerate() {
                                                    // Show drop indicator row before this position if dragging over it
                                                    if let Some(drop_idx) = drop_target_index() {
                                                        if drop_idx == index {
                                                            tr { style: "height: 1px; background-color: #3273dc;",
                                                                td {
                                                                    colspan: "5",
                                                                    style: "padding: 0; margin: 0; height: 1px;",
                                                                }
                                                                // Add invisible drop zone at the end for dropping after the last item
                                                                div {
                                                                    style: "height: 20px; width: 100%; backgroud-color: red",
                                                                    ondragover: move |evt| {
                                                                        evt.prevent_default();
                                                                        drop_target_index.set(Some(playlist().len()));
                                                                    },
                                                                    ondragleave: move |_| {
                                                                        drop_target_index.set(None);
                                                                    },
                                                                    ondrop: move |evt| {
                                                                        evt.prevent_default();
                                                                        if let Some(dragged_idx) = dragged_index() {
                                                                            if dragged_idx != playlist().len() {
                                                                                let mut current_playlist = playlist();
                                                                                let dragged_item = current_playlist[dragged_idx].clone();
                                                                                current_playlist.remove(dragged_idx);

                                                                                // Insert at the end
                                                                                current_playlist.push(dragged_item);
                                                                                playlist.set(current_playlist);
                                                                            }
                                                                        }
                                                                        dragged_index.set(None);
                                                                        drop_target_index.set(None);
                                                                    },
                                                                }
                                                            }

                                                            // Show drop indicator at the end if dragging over the last position
                                                            if let Some(drop_idx) = drop_target_index() {
                                                                if drop_idx == playlist().len() {
                                                                    tr { style: "height: 1px; background-color: #3273dc;",
                                                                        td {
                                                                            colspan: "5",
                                                                            style: "padding: 0; margin: 0; height: 1px;",
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }

                                                    tr {
                                                        draggable: "true",
                                                        ondragstart: move |_| {
                                                            dragged_index.set(Some(index));
                                                            drop_target_index.set(None);
                                                        },
                                                        ondragend: move |_| {
                                                            dragged_index.set(None);
                                                            drop_target_index.set(None);
                                                        },
                                                        ondragover: move |evt| {
                                                            evt.prevent_default();
                                                            drop_target_index.set(Some(index));
                                                        },
                                                        ondragleave: move |_| {
                                                            drop_target_index.set(None);
                                                        },
                                                        ondrop: move |evt| {
                                                            evt.prevent_default();
                                                            if let Some(dragged_idx) = dragged_index() {
                                                                if dragged_idx != index {
                                                                    let mut current_playlist = playlist();
                                                                    let dragged_item = current_playlist[dragged_idx].clone();
                                                                    current_playlist.remove(dragged_idx);

                                                                    // Calculate correct insert position

                                                                    let insert_index = if dragged_idx < index { index - 1 } else { index };
                                                                    current_playlist.insert(insert_index, dragged_item);
                                                                    playlist.set(current_playlist);
                                                                }
                                                            }
                                                            dragged_index.set(None);
                                                            drop_target_index.set(None);
                                                        },
                                                        td {
                                                            div { style: "cursor: grab; margin-right: 8px; display: inline-block;",
                                                                dioxus_free_icons::Icon {
                                                                    icon: FaGripVertical,
                                                                    width: 16,
                                                                    height: 16,
                                                                }
                                                            }
                                                        }
                                                        td {

                                                            {get_file_info(&files_resource, file_name, "artist")}
                                                        }
                                                        td {

                                                            {get_file_info(&files_resource, file_name, "title")}
                                                        }
                                                        td {
                                                            {
                                                                let duration = get_file_info(&files_resource, file_name, "duration");
                                                                if let Ok(dur) = duration.parse::<u32>() {
                                                                    let minutes = dur / 60;
                                                                    let seconds = dur % 60;
                                                                    format!("{}:{:02}", minutes, seconds)
                                                                } else {
                                                                    "Unknown".to_string()
                                                                }
                                                            }
                                                        }
                                                        td {
                                                            ControlsButton {
                                                                icon: FaMinus,
                                                                label: "Remove".to_string(),
                                                                size: Some(b::BulmaSize::Small),
                                                                onclick: Some(
                                                                    EventHandler::new(move |_| {
                                                                        let mut current_playlist = playlist();
                                                                        current_playlist.remove(index);
                                                                        playlist.set(current_playlist);
                                                                    }),
                                                                ),
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if let Some(status) = save_status() {
                                        Notification { 
                                            color: save_status_type(), "{status}" 
                                        }
                                    }
                                    b::Button {
                                        class: "is-button-gradient-primary",
                                        color: b::BulmaColor::Primary,
                                        disabled: last_fob().is_none() || playlist().is_empty(),
                                        onclick: {
                                            move |_| {
                                                let fob = last_fob().unwrap_or_default();
                                                let current_playlist = playlist();
                                                spawn(async move {
                                                    if let Err(e) = services::fob::associate_fob(&fob, &current_playlist)
                                                        .await
                                                    {
                                                        toast
                                                            .show_error(
                                                                format!("Failed to associate playlist with FOB: {}", e),
                                                            );
                                                    } else {
                                                        toast
                                                            .show_success(
                                                                format!(
                                                                    "Successfully associated {} files with FOB",
                                                                    current_playlist.len(),
                                                                ),
                                                            );
                                                    }
                                                });
                                            }
                                        },
                                        "Save Playlist"
                                    }
                                }
                            }
                        }
                        b::Column { size: b::ColumnSize::Half,
                            b::Card {
                                class: "has-background-primary-soft is-card-hover-lift",
                                b::CardHeader {
                                    class: "is-card-header-gradient",
                                    b::CardHeaderTitle { "All Files" }
                                }
                                b::CardContent {
                                    if let Some(vec) = files_resource.read().as_ref().map(|r| (**r).to_vec()) {
                                        b::Table { fullwidth: true,
                                            thead {
                                                tr {
                                                    th { "Artist" }
                                                    th { "Title" }
                                                    th { "Album" }
                                                    th { "Duration" }
                                                    th { "" }
                                                }
                                            }
                                            tbody {
                                                for entry in vec {
                                                    tr {
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
                                                        td {
                                                            ControlsButton {
                                                                icon: FaPlus,
                                                                label: "Add".to_string(),
                                                                size: Some(b::BulmaSize::Small),
                                                                onclick: Some(
                                                                    EventHandler::new(move |_| {
                                                                        let mut current_playlist = playlist();
                                                                        current_playlist.push(entry.name.clone());
                                                                        playlist.set(current_playlist);
                                                                    }),
                                                                ),
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        div { class: "box has-text-grey has-border",
                                            "Loading files..."
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
