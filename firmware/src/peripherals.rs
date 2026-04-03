use esp_hal::gpio::{AnyPin, Level};
use esp_hal::peripherals::DMA_CH0;
use esp_hal::peripherals::DMA_CH1;
use esp_hal::peripherals::I2S0;
use esp_hal::peripherals::SPI2;
use esp_hal::peripherals::SW_INTERRUPT;
use esp_hal::peripherals::TIMG0;
use esp_hal::peripherals::WIFI;

pub struct Peripherals {
    pub spi: SpiPins,
    pub sd_cs: AnyPin<'static>,
    pub rfid_cs: AnyPin<'static>,
    pub rfid_irq: AnyPin<'static>,
    pub rfid_enable: Option<(AnyPin<'static>, Level)>,
    pub audio_enable: Option<(AnyPin<'static>, Level)>,
    pub charger: ChargerPins,
    pub player: PlayerPins,
    pub controls: ControlsPins,

    pub radio: RadioPins,
    pub radio_wifi: WIFI<'static>,
    pub spi_spi2: SPI2<'static>,
    pub spi_dma: DMA_CH1<'static>,
    pub player_i2s: I2S0<'static>,
    pub player_dma: DMA_CH0<'static>,

    pub timer0: TIMG0<'static>,
    pub sw_interrupt: SW_INTERRUPT<'static>,
}

pub struct SpiPins {
    pub sck: AnyPin<'static>,
    pub mosi: AnyPin<'static>,
    pub miso: AnyPin<'static>,
}

pub struct ChargerPins {
    pub pin: AnyPin<'static>,
    pub connected_level: Level,
}

pub struct PlayerPins {
    pub bclk: AnyPin<'static>,
    pub ws: AnyPin<'static>,
    pub dout: AnyPin<'static>,
}

pub struct ControlsPins {
    pub btn_a: AnyPin<'static>,
    pub btn_b: AnyPin<'static>,
    pub btn_c: AnyPin<'static>,
    pub btn_d: AnyPin<'static>,
}

pub struct RadioPins {
    pub pin: AnyPin<'static>,
}

pub fn create_peripherals(peripherals: esp_hal::peripherals::Peripherals) -> Peripherals {
    #[cfg(feature = "pinout_pcb")]
    let is_pcb = true;
    #[cfg(not(feature = "pinout_pcb"))]
    let is_pcb = false;

    let p = peripherals;

    let (spi_sck, spi_mosi, sd_cs, rfid_enable, audio_enable, charger_level) = if is_pcb {
        (
            p.GPIO7.into(),
            p.GPIO6.into(),
            p.GPIO20.into(),
            Some((p.GPIO14.into(), Level::High)),
            Some((p.GPIO9.into(), Level::High)),
            Level::Low,
        )
    } else {
        (
            p.GPIO6.into(),
            p.GPIO7.into(),
            p.GPIO10.into(),
            None,
            None,
            Level::High,
        )
    };

    Peripherals {
        spi: SpiPins {
            sck: spi_sck,
            mosi: spi_mosi,
            miso: p.GPIO5.into(),
        },
        sd_cs,
        rfid_cs: p.GPIO18.into(),
        rfid_irq: p.GPIO19.into(),
        rfid_enable,
        audio_enable,
        charger: ChargerPins {
            pin: p.GPIO4.into(),
            connected_level: charger_level,
        },
        player: PlayerPins {
            bclk: p.GPIO23.into(),
            ws: p.GPIO15.into(),
            dout: p.GPIO22.into(),
        },
        controls: ControlsPins {
            btn_a: p.GPIO0.into(),
            btn_b: p.GPIO1.into(),
            btn_c: p.GPIO2.into(),
            btn_d: p.GPIO3.into(),
        },
        radio: RadioPins {
            pin: p.GPIO8.into(),
        },
        radio_wifi: p.WIFI,
        spi_spi2: p.SPI2,
        spi_dma: p.DMA_CH1,
        player_i2s: p.I2S0,
        player_dma: p.DMA_CH0,
        timer0: p.TIMG0,
        sw_interrupt: p.SW_INTERRUPT,
    }
}
