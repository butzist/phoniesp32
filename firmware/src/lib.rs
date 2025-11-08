#![no_std]
#![recursion_limit = "256"]
#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]
#![feature(coroutines)]
#![feature(coroutine_trait)]
#![feature(stmt_expr_attributes)]
#![feature(atomic_try_update)]
#![feature(try_blocks)]

use core::future::Future;

use alloc::string::String;
use defmt::{error, warn};
use serde::{Deserialize, Serialize};

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

/// Extends lifetime of a reference to 'static
///
/// # Safety
///
/// Intrinsically unsafe! Only use this, if you know what you are doing, r is not
/// deallocated, and you don't anticipate data races.
pub unsafe fn extend_to_static<T>(r: &mut T) -> &'static mut T {
    let ptr = r as *mut T;
    &mut *ptr
}

/// Allows to await an Option<Future>
pub async fn opt_async<F, R>(f: Option<F>) -> Option<R>
where
    F: Future<Output = R>,
{
    Some(f?.await)
}

pub fn with_extension(basename: &str, ext: &str) -> Result<heapless::String<12>, ()> {
    let mut fname = heapless::String::<12>::new();
    fname.push_str(basename)?;
    fname.push_str(ext)?;
    Ok(fname)
}

pub mod controls;
pub mod entities;
pub mod player;
pub mod radio;
pub mod rfid;
pub mod sd;
pub mod web;
