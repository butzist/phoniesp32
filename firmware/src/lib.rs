#![no_std]
#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]

use alloc::string::String;
use serde::{Deserialize, Serialize};

pub mod sd;
pub mod web;
pub mod wifi;

extern crate alloc;

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
