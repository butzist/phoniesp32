use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};

pub enum LedCommand {
    On,
    Off,
}

pub struct IndicatorLed {
    pin: AnyPin<'static>,
}

impl IndicatorLed {
    pub fn new(pin: AnyPin<'static>) -> Self {
        Self { pin }
    }

    pub fn spawn(self, spawner: &Spawner) -> IndicatorLedHandle {
        let cmd_channel = mk_static!(Channel<NoopRawMutex, LedCommand, 4>, Channel::new());
        let pin = Output::new(self.pin, Level::Low, OutputConfig::default());
        spawner.must_spawn(indicator_led_task(pin, cmd_channel));
        IndicatorLedHandle { tx: cmd_channel }
    }
}

#[derive(Clone)]
pub struct IndicatorLedHandle {
    tx: &'static Channel<NoopRawMutex, LedCommand, 4>,
}

impl IndicatorLedHandle {
    pub async fn on(&self) {
        self.tx.send(LedCommand::On).await;
    }

    pub async fn off(&self) {
        self.tx.send(LedCommand::Off).await;
    }
}

#[embassy_executor::task]
async fn indicator_led_task(
    mut pin: Output<'static>,
    rx: &'static Channel<NoopRawMutex, LedCommand, 4>,
) {
    loop {
        match rx.receive().await {
            LedCommand::On => pin.set_high(),
            LedCommand::Off => pin.set_low(),
        }
    }
}
