use embassy_time::Duration;
use esp_hal::gpio::{AnyPin, Event, Input, InputConfig, Pull, WaitForOptions, WakeEvent};

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

    pub fn is_pressed(&self) -> bool {
        self.input.is_low()
    }

    pub async fn wait_for_release(&mut self) {
        self.input
            .wait_for_with_options(
                Event::HighLevel,
                WaitForOptions::default().with_wake_enable(true),
            )
            .await
            .unwrap();
    }

    /// Wait for press and return press type
    /// For buttons with repeat enabled, returns immediately on press
    /// For other buttons, waits for release to determine short/long press
    pub async fn wait_for_press(&mut self) -> PressType {
        self.input
            .wait_for_with_options(
                Event::HighLevel,
                WaitForOptions::default().with_wake_enable(true),
            )
            .await
            .unwrap(); // Make sure button is reset
        self.input.wakeup_enable(true, WakeEvent::LowLevel).unwrap();
        embassy_time::Timer::after(Duration::from_millis(50)).await; // Debounce
        self.input
            .wait_for_with_options(
                Event::LowLevel,
                WaitForOptions::default().with_wake_enable(true),
            )
            .await
            .unwrap(); // Wait for press
        self.input
            .wakeup_enable(true, WakeEvent::HighLevel)
            .unwrap();
        embassy_time::Timer::after(Duration::from_millis(50)).await; // Debounce

        if let Some(long_press_threshold) = self.long_press_threshold {
            // Wait for release to determine press duration
            match embassy_time::with_timeout(
                long_press_threshold,
                self.input.wait_for_with_options(
                    Event::HighLevel,
                    WaitForOptions::default().with_wake_enable(true),
                ),
            )
            .await
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
    pub async fn check_repeat(&mut self) -> bool {
        embassy_time::with_timeout(
            self.repeat_interval.unwrap_or(Duration::from_millis(500)),
            self.input.wait_for_with_options(
                Event::HighLevel,
                WaitForOptions::default().with_wake_enable(true),
            ),
        )
        .await
        .is_err() // Not released
    }
}

#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum PressType {
    Short,
    Long,
}
