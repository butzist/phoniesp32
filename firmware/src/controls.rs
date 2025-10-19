use embassy_futures::select::{select4, Either4};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::Timer;
use esp_hal::gpio::TouchPin;
use esp_hal::peripherals::{self, LPWR, TOUCH};
use esp_hal::touch::{Continuous, TouchPad};
use esp_hal::Async;
use esp_hal::{rtc_cntl::Rtc, touch::Touch};

use crate::player::PlayerCommand;

pub enum AnyTouchPin<'a> {
    GPIO15(peripherals::GPIO15<'a>),
    GPIO2(peripherals::GPIO2<'a>),
    GPIO0(peripherals::GPIO0<'a>),
    GPIO4(peripherals::GPIO4<'a>),
    GPIO13(peripherals::GPIO13<'a>),
    GPIO12(peripherals::GPIO12<'a>),
    GPIO14(peripherals::GPIO14<'a>),
    GPIO27(peripherals::GPIO27<'a>),
    GPIO33(peripherals::GPIO33<'a>),
    GPIO32(peripherals::GPIO32<'a>),
}

impl<'a> AnyTouchPin<'a> {
    async fn wait_for_touch(&mut self, touch: &Touch<'_, Continuous, Async>) {
        match self {
            AnyTouchPin::GPIO15(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO2(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO0(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO4(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO13(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO12(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO14(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO27(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO33(p) => wait_for_touch(p.reborrow(), touch).await,
            AnyTouchPin::GPIO32(p) => wait_for_touch(p.reborrow(), touch).await,
        }
    }
}

async fn wait_for_touch(pin: impl TouchPin, touch: &Touch<'_, Continuous, Async>) {
    let mut pad = TouchPad::new(pin, touch);
    pad.wait_for_touch(100).await;
}

pub struct Controls {
    rtc: Rtc<'static>,
    touch: TOUCH<'static>,
    pin1: AnyTouchPin<'static>,
    pin2: AnyTouchPin<'static>,
    pin3: AnyTouchPin<'static>,
    pin4: AnyTouchPin<'static>,
}

impl Controls {
    pub fn new(
        rtc: LPWR<'static>,
        touch: TOUCH<'static>,
        pin1: AnyTouchPin<'static>,
        pin2: AnyTouchPin<'static>,
        pin3: AnyTouchPin<'static>,
        pin4: AnyTouchPin<'static>,
    ) -> Self {
        Self {
            rtc: Rtc::new(rtc),
            touch,
            pin1,
            pin2,
            pin3,
            pin4,
        }
    }

    async fn wait_for_touch(&mut self) -> Pad {
        let touch = Touch::async_mode(self.touch.reborrow(), &mut self.rtc, None);

        match select4(
            self.pin1.wait_for_touch(&touch),
            self.pin2.wait_for_touch(&touch),
            self.pin3.wait_for_touch(&touch),
            self.pin4.wait_for_touch(&touch),
        )
        .await
        {
            Either4::First(_) => Pad::Play,
            Either4::Second(_) => Pad::Stop,
            Either4::Third(_) => Pad::VolumeUp,
            Either4::Fourth(_) => Pad::VolumeDown,
        }
    }
}

enum Pad {
    Play,
    Stop,
    VolumeUp,
    VolumeDown,
}

#[embassy_executor::task]
pub async fn run_controls(
    mut controls: Controls,
    commands: Sender<'static, NoopRawMutex, PlayerCommand, 2>,
) {
    loop {
        match controls.wait_for_touch().await {
            Pad::Play => {
                commands
                    .send(PlayerCommand::Play("test.wav".try_into().unwrap()))
                    .await
            }
            Pad::Stop => commands.send(PlayerCommand::Stop).await,
            Pad::VolumeUp => commands.send(PlayerCommand::VolumeUp).await,
            Pad::VolumeDown => commands.send(PlayerCommand::VolumeDown).await,
        }
        Timer::after_secs(1).await;
    }
}
