#![no_std]
#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]
#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes)]

use alloc::string::String;
use defmt::{error, warn};
use serde::{Deserialize, Serialize};

pub mod player;
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

pub async fn retry<F, R, E>(mut f: F, times: u8) -> Result<R, E>
where
    F: AsyncFnMut() -> Result<R, E>,
{
    for _ in 0..times - 1 {
        if let Ok(val) = f().await {
            return Ok(val);
        }
    }

    f().await
}

pub trait PrintErr<T> {
    fn print_warn(self, message: &str) -> Option<T>;
    fn print_err(self, message: &str) -> Option<T>;
}

impl<T, E> PrintErr<T> for Result<T, E>
where
    E: defmt::Format,
{
    fn print_warn(self, message: &str) -> Option<T> {
        match self {
            Ok(val) => Some(val),
            Err(err) => {
                warn!("{}: {}", message, err);
                None
            }
        }
    }

    fn print_err(self, message: &str) -> Option<T> {
        match self {
            Ok(val) => Some(val),
            Err(err) => {
                error!("{}: {}", message, err);
                None
            }
        }
    }
}
