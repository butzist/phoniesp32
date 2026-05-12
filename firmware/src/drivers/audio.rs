use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;

use crate::PrintErr;
use crate::controllers::playback::status::{State, Status};
use crate::extend_to_static;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Sender as EmbSender;
use esp_hal::dma::DmaDescriptor;
use esp_hal::gpio::{Level, Output};
use esp_hal::i2s::master::{I2s, asynch::I2sWriteDmaTransferAsync};

extern crate alloc;

// ---- Audio channel types ----

pub const BUF_SAMPLES: usize = 2041;

pub struct AudioBuffer {
    pub samples: [i16; BUF_SAMPLES],
    pub len: usize,
}

impl AudioBuffer {
    pub fn alloc() -> Box<Self> {
        Box::new(Self {
            samples: [0i16; BUF_SAMPLES],
            len: 0,
        })
    }
}

pub enum AudioPacket {
    Buffer(Box<AudioBuffer>),
    Silence(u32),
    Eof,
}

pub type AudioSender = EmbSender<'static, NoopRawMutex, AudioPacket, 1>;

// ---- I2S / Player ----

pub(crate) const DMA_SIZE: usize = 6 * 4096;
const DMA_CHUNKS: usize = 7;

pub(crate) struct I2sInner {
    i2s: esp_hal::i2s::AnyI2s<'static>,
    dma: esp_hal::peripherals::DMA_CH0<'static>,
    bclk: esp_hal::gpio::AnyPin<'static>,
    ws: esp_hal::gpio::AnyPin<'static>,
    dout: esp_hal::gpio::AnyPin<'static>,
    dma_buffer: &'static mut [u8; DMA_SIZE],
    dma_descriptors: &'static mut [DmaDescriptor; DMA_CHUNKS],
}

impl I2sInner {
    fn transfer(&mut self) -> I2sWriteDmaTransferAsync<'_, &mut [u8; DMA_SIZE]> {
        let config = esp_hal::i2s::master::Config::new_tdm_philips()
            .with_sample_rate(esp_hal::time::Rate::from_hz(44100))
            .with_data_format(esp_hal::i2s::master::DataFormat::Data16Channel16);

        let i2s = I2s::new(self.i2s.reborrow(), self.dma.reborrow(), config)
            .unwrap()
            .into_async();

        // SAFETY: will actually only be borrowed for the lifetime of the transfer
        let reborrowed_dma_descriptors = unsafe { extend_to_static(self.dma_descriptors) };

        let i2s_tx = i2s
            .i2s_tx
            .with_bclk(self.bclk.reborrow())
            .with_ws(self.ws.reborrow())
            .with_dout(self.dout.reborrow())
            .build(reborrowed_dma_descriptors);

        i2s_tx
            .write_dma_circular_async::<&mut [u8; DMA_SIZE]>(self.dma_buffer)
            .unwrap()
    }
}

pub struct Player {
    i2s_inner: I2sInner,
    audio_enable: &'static mut Option<(Output<'static>, Level)>,
}

impl Player {
    pub fn new(
        i2s: esp_hal::i2s::AnyI2s<'static>,
        dma: esp_hal::peripherals::DMA_CH0<'static>,
        bclk: esp_hal::gpio::AnyPin<'static>,
        ws: esp_hal::gpio::AnyPin<'static>,
        dout: esp_hal::gpio::AnyPin<'static>,
        audio_enable_pin: Option<(esp_hal::gpio::AnyPin<'static>, Level)>,
    ) -> Self {
        let (_, _, tx_buffer, tx_descriptors) = esp_hal::dma_buffers!(0, DMA_SIZE);

        let audio_enable = crate::mk_static!(
            Option<(Output<'static>, Level)>,
            audio_enable_pin.map(|(pin, active_level)| {
                (
                    Output::new(pin, !active_level, esp_hal::gpio::OutputConfig::default()),
                    active_level,
                )
            })
        );

        Self {
            i2s_inner: I2sInner {
                i2s,
                dma,
                bclk,
                ws,
                dout,
                dma_buffer: tx_buffer,
                dma_descriptors: tx_descriptors,
            },
            audio_enable,
        }
    }

    pub fn set_amp(&mut self, enabled: bool) {
        if let Some((pin, level)) = self.audio_enable {
            pin.set_level(if enabled { *level } else { !*level });
        }
    }

    pub fn amp_active_level(&self) -> Level {
        self.audio_enable
            .as_ref()
            .map(|(_, l)| *l)
            .unwrap_or(Level::Low)
    }

    pub fn start(
        player: Rc<RefCell<Self>>,
        spawner: &Spawner,
        status: &'static Status,
    ) -> AudioSender {
        let channel = crate::mk_static!(
            Channel<NoopRawMutex, AudioPacket, 1>,
            Channel::new()
        );
        let sender = channel.sender();

        let mut this = player.borrow_mut();
        let transfer = this.i2s_inner.transfer();
        // SAFETY: All data referenced by the transfer is either:
        // 1. DMA buffer + descriptors — allocated in static memory by dma_buffers!().
        // 2. I2S peripheral, DMA channel, GPIO pins — memory-mapped hardware at
        //    fixed addresses; AnyI2s<'static>/DMA_CH0<'static>/AnyPin<'static> are
        //    zero-sized types whose 'static lifetime reflects hardware persistence.
        // 3. The Player (owner of all above) is kept alive via Rc passed to
        //    i2s_output_task, guaranteeing validity for the transfer's lifetime.
        let transfer = unsafe {
            core::mem::transmute::<
                I2sWriteDmaTransferAsync<'_, &mut [u8; DMA_SIZE]>,
                I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
            >(transfer)
        };
        drop(this);

        spawner.must_spawn(i2s_output_task(transfer, player, channel, status));
        sender
    }
}

#[embassy_executor::task]
async fn i2s_output_task(
    mut transfer: I2sWriteDmaTransferAsync<'static, &'static mut [u8; DMA_SIZE]>,
    player: Rc<RefCell<Player>>,
    channel: &'static Channel<NoopRawMutex, AudioPacket, 1>,
    status: &'static Status,
) {
    let mut amp_enabled = false;
    let rx = channel.receiver();

    loop {
        match rx.receive().await {
            AudioPacket::Eof => break,

            AudioPacket::Buffer(buf) => {
                if !amp_enabled {
                    player.borrow_mut().set_amp(true);
                    amp_enabled = true;
                }

                let n = buf.len.min(BUF_SAMPLES);
                transfer
                    .push_with(|out: &mut [u8]| {
                        let max_out = out.len() / 4;
                        let count = n.min(max_out);
                        for i in 0..count {
                            let s = buf.samples[i];
                            out[i * 4] = s as u8;
                            out[i * 4 + 1] = (s >> 8) as u8;
                            out[i * 4 + 2] = 0;
                            out[i * 4 + 3] = 0;
                        }
                        count * 4
                    })
                    .await
                    .print_err("Player: I2S DMA transfer");
            }

            AudioPacket::Silence(samples) => {
                if amp_enabled {
                    player.borrow_mut().set_amp(false);
                    amp_enabled = false;
                }

                let mut remaining = samples as usize;
                while remaining > 0 {
                    let n = remaining.min(DMA_SIZE / 4);
                    transfer
                        .push_with(|out: &mut [u8]| {
                            let bytes = n * 4;
                            out[..bytes].fill(0);
                            bytes
                        })
                        .await
                        .print_err("Player: I2S DMA transfer");
                    remaining -= n;
                }
            }
        }
    }

    if amp_enabled {
        player.borrow_mut().set_amp(false);
    }

    status.update_state(State::Stopped);
}
