use crate::decode::DecoderError;
use crate::resample::ResampleError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TranscodeError {
    #[error("decoding failed")]
    Decode(#[from] DecoderError),
    #[error("I/O error")]
    IOError(#[from] std::io::Error),
    #[error("resampling failed")]
    ResampleError(#[from] ResampleError),
    #[error("no supported audio tracks")]
    NoAudioTracks,
    #[error("unknown input sample rate")]
    UnknownSampleRate,
    #[error("unknown channels count")]
    UnknownChannelsCount,
}
