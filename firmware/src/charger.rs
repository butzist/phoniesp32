use defmt::info;
use embassy_executor::Spawner;
use embassy_sync::watch::Watch;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Receiver};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

const NUM_RECEIVERS: usize = 2;

#[derive(Clone, Copy, PartialEq)]
pub enum ChargerState {
    Connected,
    Disconnected,
}

pub struct Charger {
    pin: Input<'static>,
    state: &'static Watch<CriticalSectionRawMutex, ChargerState, NUM_RECEIVERS>,
}

impl Charger {
    pub fn new(pin: AnyPin<'static>) -> Self {
        let pin = Input::new(pin, InputConfig::default().with_pull(Pull::Up));

        // Set initial state
        let initial_state = Self::current_state(&pin);
        let watch = mk_static!(Watch<CriticalSectionRawMutex, ChargerState, NUM_RECEIVERS>, Watch::new_with(initial_state));

        info!(
            "Initial charger state: {}",
            if initial_state == ChargerState::Connected {
                "connected"
            } else {
                "disconnected"
            }
        );

        Self { pin, state: watch }
    }

    fn current_state(pin: &Input<'static>) -> ChargerState {
        let is_connected = pin.is_high();
        if is_connected {
            ChargerState::Connected
        } else {
            ChargerState::Disconnected
        }
    }

    async fn run(mut self) {
        let sender = self.state.sender();
        let mut last_connected = Self::current_state(&self.pin);

        loop {
            let current_connected = Self::current_state(&self.pin);

            if current_connected != last_connected {
                info!(
                    "Charger event: {}",
                    if current_connected == ChargerState::Connected {
                        "plugged in"
                    } else {
                        "unplugged"
                    }
                );
                // Notify watchers
                sender.send(current_connected);
                last_connected = current_connected;

                // Debounce
                Timer::after(Duration::from_millis(200)).await;
            } else {
                self.pin.wait_for_any_edge().await;
            }
        }
    }

    fn monitor(&self) -> ChargerMonitor {
        // This expect will not panic because: NUM_RECEIVERS = 2 and currently only the radio connection task creates a monitor
        ChargerMonitor::new(self.state)
    }

    pub fn spawn(self, spawner: &Spawner) -> ChargerMonitor {
        let monitor = self.monitor();
        spawner.spawn(charger_task(self)).ok();
        monitor
    }
}

pub struct ChargerMonitor {
    watch: &'static Watch<CriticalSectionRawMutex, ChargerState, NUM_RECEIVERS>,
    receiver: Receiver<'static, CriticalSectionRawMutex, ChargerState, NUM_RECEIVERS>,
}

impl Clone for ChargerMonitor {
    fn clone(&self) -> Self {
        // SAFETY: will not panic because: NUM_RECEIVERS = 2 and currently only the radio connection task creates a monitor
        let receiver = self.watch.receiver().unwrap();
        Self {
            watch: self.watch,
            receiver,
        }
    }
}

impl ChargerMonitor {
    fn new(watch: &'static Watch<CriticalSectionRawMutex, ChargerState, NUM_RECEIVERS>) -> Self {
        // SAFETY: will not panic because: NUM_RECEIVERS = 2 and currently only the radio connection task creates a monitor
        let receiver = watch.receiver().unwrap();
        Self { watch, receiver }
    }

    pub fn is_connected(&mut self) -> bool {
        self.receiver.try_get() == Some(ChargerState::Connected)
    }

    pub async fn wait_for_event(&mut self) -> ChargerState {
        self.receiver.changed().await
    }
}

#[embassy_executor::task]
pub async fn charger_task(charger: Charger) {
    charger.run().await;
}
