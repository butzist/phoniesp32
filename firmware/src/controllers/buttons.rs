use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::select::{Either4, select, select4};
use embassy_time::{Duration, Timer};

use crate::controllers::playback::PlaybackHandle;
use crate::controllers::wifi::WifiManagerHandle;
use crate::drivers::control_button::{Button, PressType};

pub struct Buttons {
    play_pause: Button,
    next_prev: Button,
    volume_down: Button,
    volume_up: Button,
}

impl Buttons {
    pub fn new(
        play_pause: Button,
        next_prev: Button,
        volume_down: Button,
        volume_up: Button,
    ) -> Self {
        Self {
            play_pause,
            next_prev,
            volume_down,
            volume_up,
        }
    }

    async fn wait_for_both_volume_held(&mut self) -> bool {
        embassy_time::with_timeout(
            Duration::from_secs(3),
            select(
                self.volume_up.wait_for_release(),
                self.volume_down.wait_for_release(),
            ),
        )
        .await
        .is_err() // timed out = both held for 3s
    }

    async fn wait_for_event(&mut self) -> ControlEvent {
        match select4(
            self.play_pause.wait_for_press(),
            self.next_prev.wait_for_press(),
            self.volume_up.wait_for_press(),
            self.volume_down.wait_for_press(),
        )
        .await
        {
            Either4::First(press_type) => ControlEvent::PlayPause(press_type),
            Either4::Second(press_type) => ControlEvent::NextPrev(press_type),
            Either4::Third(_) => {
                if self.volume_down.is_pressed() && self.wait_for_both_volume_held().await {
                    return ControlEvent::WifiOn;
                }
                ControlEvent::VolumeUp
            }
            Either4::Fourth(_) => {
                if self.volume_up.is_pressed() && self.wait_for_both_volume_held().await {
                    return ControlEvent::WifiOn;
                }
                ControlEvent::VolumeDown
            }
        }
    }

    pub fn spawn(self, spawner: &Spawner, wifi_handle: WifiManagerHandle, player: PlaybackHandle) {
        spawner.must_spawn(buttons_task(self, player, wifi_handle));
    }
}

enum ControlEvent {
    PlayPause(PressType),
    NextPrev(PressType),
    VolumeUp,
    VolumeDown,
    WifiOn,
}

#[embassy_executor::task]
async fn buttons_task(
    mut buttons: Buttons,
    player: PlaybackHandle,
    wifi_handle: WifiManagerHandle,
) {
    loop {
        match buttons.wait_for_event().await {
            ControlEvent::PlayPause(press_type) => {
                info!("Buttons: PlayPause({:?})", press_type);
                match press_type {
                    PressType::Short => player.pause().await,
                    PressType::Long => player.stop().await,
                }
            }
            ControlEvent::NextPrev(press_type) => {
                info!("Buttons: NextPrev({:?})", press_type);
                match press_type {
                    PressType::Short => player.skip_next().await,
                    PressType::Long => player.skip_previous().await,
                }
            }
            ControlEvent::VolumeUp => {
                info!("Buttons: VolumeUp");
                player.volume_up().await;
                // Continue repeating while held
                while buttons.volume_up.check_repeat().await {
                    info!("Buttons: VolumeUp (repeat)");
                    player.volume_up().await;
                }
            }
            ControlEvent::VolumeDown => {
                info!("Buttons: VolumeDown");
                player.volume_down().await;
                // Continue repeating while held
                while buttons.volume_down.check_repeat().await {
                    info!("Buttons: VolumeDown (repeat)");
                    player.volume_down().await;
                }
            }
            ControlEvent::WifiOn => {
                info!("Buttons: WiFi override on");
                wifi_handle.wifi_on().await;
            }
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}
