use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Level, Pull};
use esp_println::println;

pub enum ChargerEvent {
    Connected,
    Disconnected,
}

pub struct ChargerMonitor {
    pin: Input<'static>,
    signal: Signal<CriticalSectionRawMutex, Level>,
}

impl ChargerMonitor {
    pub fn new(pin: AnyPin<'static>) -> Self {
        let pin = Input::new(pin, InputConfig::default().with_pull(Pull::None));
        let signal = Signal::new();
        Self { pin, signal }
    }

    pub fn is_connected(&self) -> bool {
        self.pin.is_high()
    }

    pub async fn wait_for_event(&self) -> ChargerEvent {
        match self.signal.wait().await {
            Level::Low => ChargerEvent::Disconnected,
            Level::High => ChargerEvent::Connected,
        }
    }

    pub async fn monitor(&mut self) {
        let mut last_level = self.pin.level();
        self.signal.signal(last_level);

        println!(
            "Initial charger state: {}",
            if last_level == Level::High {
                "connected"
            } else {
                "disconnected"
            }
        );

        loop {
            self.pin.wait_for_any_edge().await;
            let current_state = self.pin.level();

            if current_state != last_level {
                println!(
                    "Charger event: {}",
                    if current_state == Level::High {
                        "plugged in"
                    } else {
                        "unplugged"
                    }
                );
                self.signal.signal(current_state);
                last_level = current_state;
            }

            Timer::after(Duration::from_millis(500)).await;
        }
    }
}

#[embassy_executor::task]
pub async fn charger_monitor_task(mut charger: Charger) {
    charger.monitor().await;
}

pub struct Charger {}

impl Charger {
    pub fn new(pin: AnyPin<'static>) -> Self {
        let pin = Input::new(pin, InputConfig::default().with_pull(Pull::None));
        let signal = Signal::new();
        Self { pin, signal }
    }

    pub fn is_connected(&self) -> bool {
        self.pin.is_high()
    }

    pub async fn wait_for_event(&self) -> ChargerEvent {
        match self.signal.wait().await {
            Level::Low => ChargerEvent::Disconnected,
            Level::High => ChargerEvent::Connected,
        }
    }
}
