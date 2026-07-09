// Shared test utilities for firmware tests.

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use firmware::mk_static;

// ---- Test command channels for controller mocking ----

#[derive(Clone, Copy, PartialEq, defmt::Format)]
pub enum TestWifiCmd {
    On,
    Off,
}

/// A test fake that mimics the WifiManagerHandle interface,
/// recording commands sent to it.
pub struct TestWifiHandle {
    cmd_channel: &'static Channel<NoopRawMutex, TestWifiCmd, 4>,
    stack_ready: &'static Signal<NoopRawMutex, ()>,
}

impl TestWifiHandle {
    pub fn new() -> Self {
        let cmd_channel = firmware::mk_static!(
            Channel<NoopRawMutex, TestWifiCmd, 4>,
            Channel::new()
        );
        let stack_ready = firmware::mk_static!(Signal<NoopRawMutex, ()>, Signal::new());
        stack_ready.signal(());
        Self {
            cmd_channel,
            stack_ready,
        }
    }

    pub fn cmd_channel(&self) -> &'static Channel<NoopRawMutex, TestWifiCmd, 4> {
        self.cmd_channel
    }

    pub async fn wifi_on(&self) {
        self.cmd_channel.send(TestWifiCmd::On).await;
    }

    pub async fn wifi_off(&self) {
        self.cmd_channel.send(TestWifiCmd::Off).await;
    }

    pub async fn wait_for_stack(&self) {
        self.stack_ready.wait().await;
    }
}

/// Collects commands sent through a channel for test assertions.
pub struct CmdCollector<C: Copy + defmt::Format, const N: usize> {
    rx: embassy_sync::channel::Receiver<'static, NoopRawMutex, C, N>,
}

impl<C: Copy + defmt::Format, const N: usize> CmdCollector<C, N> {
    pub fn new(ch: &'static Channel<NoopRawMutex, C, N>) -> Self {
        Self { rx: ch.receiver() }
    }

    pub async fn next(&mut self) -> C {
        self.rx.receive().await
    }

    pub async fn try_next_timeout(&mut self, ms: u64) -> Option<C> {
        embassy_time::with_timeout(Duration::from_millis(ms), self.rx.receive())
            .await
            .ok()
    }
}
