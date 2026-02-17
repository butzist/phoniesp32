use embassy_executor::Spawner;
use embassy_futures::select::{Either4, select4};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

use crate::player::PlayerHandle;

pub struct Button {
    input: Input<'static>,
    long_press_threshold: Option<Duration>,
    repeat_interval: Option<Duration>,
}

impl Button {
    pub fn new(pin: AnyPin<'static>) -> Self {
        let input = Input::new(pin, InputConfig::default().with_pull(Pull::Up));
        Self {
            input,
            long_press_threshold: None,
            repeat_interval: None,
        }
    }

    pub fn with_long_press(mut self, long_press_threshold_ms: u64) -> Self {
        self.long_press_threshold = Some(Duration::from_millis(long_press_threshold_ms));
        self
    }

    pub fn with_repeat(mut self, interval_ms: u64) -> Self {
        self.repeat_interval = Some(Duration::from_millis(interval_ms));
        self
    }

    /// Wait for press and return press type
    /// For buttons with repeat enabled, returns immediately on press
    /// For other buttons, waits for release to determine short/long press
    async fn wait_for_press(&mut self) -> PressType {
        self.input.wait_for_high().await; // Make sure button is reset
        Timer::after(Duration::from_millis(50)).await; // Debounce
        self.input.wait_for_low().await; // Wait for press
        Timer::after(Duration::from_millis(50)).await; // Debounce

        if let Some(long_press_threshold) = self.long_press_threshold {
            // Wait for release to determine press duration
            match embassy_time::with_timeout(long_press_threshold, self.input.wait_for_high()).await
            {
                Ok(_) => PressType::Short, // Released
                Err(_) => PressType::Long, // Still held - trigger event
            }
        } else {
            PressType::Short
        }
    }

    /// Check if button is still pressed and wait for repeat interval
    /// Returns true if should repeat, false if button was released
    async fn check_repeat(&mut self) -> bool {
        embassy_time::with_timeout(
            self.repeat_interval.unwrap_or(Duration::from_millis(500)),
            self.input.wait_for_high(),
        )
        .await
        .is_err() // Not released
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PressType {
    Short,
    Long,
}

pub struct Controls {
    play_pause: Button,
    next_prev: Button,
    volume_up: Button,
    volume_down: Button,
}

impl Controls {
    pub fn new(
        play_pause_pin: AnyPin<'static>,
        next_prev_pin: AnyPin<'static>,
        volume_up_pin: AnyPin<'static>,
        volume_down_pin: AnyPin<'static>,
    ) -> Self {
        Self {
            play_pause: Button::new(play_pause_pin).with_long_press(1500),
            next_prev: Button::new(next_prev_pin).with_long_press(1500),
            volume_up: Button::new(volume_up_pin).with_repeat(500),
            volume_down: Button::new(volume_down_pin).with_repeat(500),
        }
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
            Either4::Third(_) => ControlEvent::VolumeUp,
            Either4::Fourth(_) => ControlEvent::VolumeDown,
        }
    }

    pub fn spawn(self, spawner: &Spawner, player: PlayerHandle) {
        spawner.must_spawn(controls_task(self, player));
    }
}

enum ControlEvent {
    PlayPause(PressType),
    NextPrev(PressType),
    VolumeUp,
    VolumeDown,
}

#[embassy_executor::task]
async fn controls_task(mut controls: Controls, player: PlayerHandle) {
    loop {
        match controls.wait_for_event().await {
            ControlEvent::PlayPause(press_type) => match press_type {
                PressType::Short => player.pause().await,
                PressType::Long => player.stop().await,
            },
            ControlEvent::NextPrev(press_type) => match press_type {
                PressType::Short => player.skip_next().await,
                PressType::Long => player.skip_previous().await,
            },
            ControlEvent::VolumeUp => {
                player.volume_up().await;
                // Continue repeating while held
                while controls.volume_up.check_repeat().await {
                    player.volume_up().await;
                }
            }
            ControlEvent::VolumeDown => {
                player.volume_down().await;
                // Continue repeating while held
                while controls.volume_down.check_repeat().await {
                    player.volume_down().await;
                }
            }
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}
