use embedded_hal::spi::{ErrorType, SpiDevice as BlockingSpiDevice};
use embedded_hal_async::spi::SpiDevice as AsyncSpiDevice;

pub struct AsyncToBlockingSpiDevice<SPI> {
    async_spi: SPI,
}

impl<SPI> AsyncToBlockingSpiDevice<SPI> {
    pub fn new(async_spi: SPI) -> Self {
        Self { async_spi }
    }
}

impl<SPI> ErrorType for AsyncToBlockingSpiDevice<SPI>
where
    SPI: ErrorType,
{
    type Error = SPI::Error;
}

impl<SPI, Word> BlockingSpiDevice<Word> for AsyncToBlockingSpiDevice<SPI>
where
    SPI: AsyncSpiDevice<Word>,
    Word: Copy + 'static,
{
    fn transaction(
        &mut self,
        operations: &mut [embedded_hal::spi::Operation<'_, Word>],
    ) -> Result<(), Self::Error> {
        embassy_futures::block_on(self.async_spi.transaction(operations))
    }

    fn read(&mut self, buf: &mut [Word]) -> Result<(), Self::Error> {
        embassy_futures::block_on(self.async_spi.read(buf))
    }

    fn write(&mut self, buf: &[Word]) -> Result<(), Self::Error> {
        embassy_futures::block_on(self.async_spi.write(buf))
    }

    fn transfer(&mut self, read: &mut [Word], write: &[Word]) -> Result<(), Self::Error> {
        embassy_futures::block_on(self.async_spi.transfer(read, write))
    }

    fn transfer_in_place(&mut self, buf: &mut [Word]) -> Result<(), Self::Error> {
        embassy_futures::block_on(self.async_spi.transfer_in_place(buf))
    }
}
