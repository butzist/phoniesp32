use embassy_embedded_hal::shared_bus::asynch::spi::SpiDeviceWithConfig;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use esp_hal::dma::{AnyGdmaChannel, DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::AnyPin;
use esp_hal::spi::master::{AnySpi, Spi, SpiDmaBus};
use esp_hal::time::Rate;
use esp_hal::{Async, dma_buffers_chunk_size, spi};

pub type SharedSpi = &'static Mutex<NoopRawMutex, SpiDmaBus<'static, Async>>;

pub type SpiDevice = SpiDeviceWithConfig<
    'static,
    NoopRawMutex,
    SpiDmaBus<'static, Async>,
    esp_hal::gpio::Output<'static>,
>;

pub fn make_shared_spi(
    spi: AnySpi<'static>,
    dma: AnyGdmaChannel<'static>,
    sck: AnyPin<'static>,
    mosi: AnyPin<'static>,
    miso: AnyPin<'static>,
) -> SharedSpi {
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) =
        dma_buffers_chunk_size!(4 * 1024, 1024);

    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    let spi = Spi::new(
        spi,
        spi::master::Config::default()
            .with_frequency(Rate::from_khz(400))
            .with_mode(spi::Mode::_0),
    )
    .unwrap()
    .with_sck(sck)
    .with_mosi(mosi)
    .with_miso(miso)
    .with_dma(dma)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    mk_static!(Mutex<NoopRawMutex, SpiDmaBus<'static, Async>>, Mutex::new(spi))
}

pub fn make_spi_device(shared_bus: SharedSpi, cs: AnyPin<'static>, frequency: Rate) -> SpiDevice {
    use esp_hal::gpio::{Level, Output, OutputConfig};

    let cs_output = Output::new(cs, Level::High, OutputConfig::default());

    SpiDeviceWithConfig::new(
        shared_bus,
        cs_output,
        spi::master::Config::default()
            .with_frequency(frequency)
            .with_mode(spi::Mode::_0),
    )
}
