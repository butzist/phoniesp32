use std::time::Duration;

pub(crate) mod config;
pub(crate) mod files;
pub(crate) mod fob;
pub(crate) mod playback;
pub(crate) mod transcoder;
pub(crate) mod utils;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
