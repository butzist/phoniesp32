use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Receiver;

pub mod control;
pub mod playback;
pub mod status;

// Re-export the main types for backwards compatibility
pub use control::{PlayerCommand, handle_command};
pub use playback::{Player, get_volume, play_files, stop_player, volume_down, volume_up};

use crate::player::status::Status;

#[embassy_executor::task]
pub async fn run_player(
    spawner: embassy_executor::Spawner,
    mut player: Player,
    fs: &'static crate::sd::SdFileSystem<'static>,
    commands: Receiver<'static, NoopRawMutex, PlayerCommand, 2>,
) {
    // Initialize status watches
    let status = Status::get();

    loop {
        let command = commands.receive().await;
        handle_command(command, fs, &mut player, status, &spawner).await;
    }
}
