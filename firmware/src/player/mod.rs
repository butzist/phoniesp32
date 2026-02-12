pub mod control;
pub mod playback;
pub mod status;

// Re-export the main types for backwards compatibility

pub use control::{PlayerCommand, handle_command};
pub use playback::{Player, get_volume, play_files, stop_player, volume_down, volume_up};

use crate::player::status::Status;
