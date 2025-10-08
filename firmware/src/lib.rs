#![no_std]
#![feature(impl_trait_in_assoc_type)]

use alloc::string::String;
use serde::{Deserialize, Serialize};

pub mod web;
pub mod wifi;

extern crate alloc;

#[macro_export]
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[derive(Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(alias = "SSID")]
    pub ssid: String,

    #[serde(alias = "PASSWORD")]
    pub password: String,
}

impl defmt::Format for DeviceConfig {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "DeviceConfig {{ SSID: {}, PASSWORD: {} }}",
            self.ssid.as_str(),
            self.password.as_str(),
        )
    }
}
