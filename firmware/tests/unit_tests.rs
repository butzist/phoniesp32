//! Unit tests for pure logic, state machines, and controller interfaces.
//!
//! No hardware required beyond minimal HAL init to satisfy linker symbols.

#![no_std]
#![no_main]

use core::cell::RefCell;
use core::sync::atomic::Ordering;

use embedded_test::*;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use esp_hal::clock::CpuClock;
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::timer::timg::TimerGroup;
use firmware::controllers::playback::status::State;
use firmware::controllers::playback::{PlaybackContext, PlaybackHandle, Skip, PlayerCommand};
use firmware::controllers::playback::handle_skip;
use firmware::entities::audio_file::AudioFile;
use firmware::entities::basename;
use firmware::entities::playlist::Playlist;
use firmware::peripherals::create_peripherals;
use firmware::{mk_static, with_extension};

extern crate alloc;

// --------------- helpers (outside #[tests] mod) ---------------
fn mk_ctx() -> &'static PlaybackContext {
    mk_static!(PlaybackContext, PlaybackContext::new())
}
fn mk_chan() -> &'static Channel<NoopRawMutex, PlayerCommand, 2> {
    mk_static!(Channel<NoopRawMutex, PlayerCommand, 2>, Channel::new())
}

// --------------- minimal hardware init ---------------
fn hw_init() {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let esp_periphs = esp_hal::init(config);
    let periphs = create_peripherals(esp_periphs);

    let timer0 = TimerGroup::new(periphs.timer0);
    let sw_int = SoftwareInterruptControl::new(periphs.sw_interrupt);
    esp_rtos::start(timer0.timer0, sw_int.software_interrupt0);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);
    esp_alloc::heap_allocator!(size: 65536);

    let _rtc = mk_static!(
        RefCell<Rtc<'static>>,
        RefCell::new(Rtc::new(periphs.lpwr))
    );
}

#[tests]
mod tests {
    use super::*;

    #[init]
    fn init() {
        hw_init();
    }

    // ========================
    //  lib.rs — with_extension
    // ========================

    #[test]
    fn with_extension_basic() {
        let r = with_extension("ABCD", ".WAV").unwrap();
        assert_eq!(r.as_str(), "ABCD.WAV");
    }

    #[test]
    fn with_extension_empty_base() {
        let r = with_extension("", ".WAV").unwrap();
        assert_eq!(r.as_str(), ".WAV");
    }

    #[test]
    fn with_extension_max_capacity_ok() {
        let r = with_extension("0123456", ".WAV").unwrap();
        assert_eq!(r.as_str(), "0123456.WAV");
    }

    #[test]
    fn with_extension_overflow() {
        let r = with_extension("0123456789AB", ".WAV");
        assert!(r.is_err());
    }

    // ========================
    //  entities — basename
    // ========================

    #[test]
    fn basename_strips_correctly() {
        let r = basename(b"HELLO.WAV", ".WAV");
        assert!(r.is_some());
        assert_eq!(r.unwrap().as_str(), "HELLO");
    }

    #[test]
    fn basename_no_match() {
        let r = basename(b"HELLO.TXT", ".WAV");
        assert!(r.is_none());
    }

    #[test]
    fn basename_empty_result() {
        let r = basename(b".WAV", ".WAV");
        assert!(r.is_some());
        assert_eq!(r.unwrap().as_str(), "");
    }

    // ========================
    //  entities — AudioFile
    // ========================

    #[test]
    fn audio_file_from_path_valid() {
        let af = AudioFile::from_path("..\\FILES\\MYSONG.WAV");
        assert!(af.is_some());
        assert_eq!(af.unwrap().name(), "MYSONG");
    }

    #[test]
    fn audio_file_from_path_wrong_prefix() {
        assert!(AudioFile::from_path("..\\OTHER\\FILE.WAV").is_none());
    }

    #[test]
    fn audio_file_from_path_wrong_extension() {
        assert!(AudioFile::from_path("..\\FILES\\FILE.TXT").is_none());
    }

    // ========================
    //  controllers — handle_skip
    // ========================

    #[test]
    fn handle_skip_next_increments() {
        let mut idx = 0usize;
        handle_skip(Skip::Next, &mut idx, 5);
        assert_eq!(idx, 1);
    }

    #[test]
    fn handle_skip_next_clamps() {
        let mut idx = 4usize;
        handle_skip(Skip::Next, &mut idx, 5);
        assert_eq!(idx, 4);
    }

    #[test]
    fn handle_skip_prev_decrements() {
        let mut idx = 3usize;
        handle_skip(Skip::Previous, &mut idx, 10);
        assert_eq!(idx, 2);
    }

    #[test]
    fn handle_skip_prev_stays_at_zero() {
        let mut idx = 0usize;
        handle_skip(Skip::Previous, &mut idx, 5);
        assert_eq!(idx, 0);
    }

    #[test]
    fn handle_skip_single_file() {
        let mut idx = 0usize;
        handle_skip(Skip::Next, &mut idx, 1);
        assert_eq!(idx, 0);
        handle_skip(Skip::Previous, &mut idx, 1);
        assert_eq!(idx, 0);
    }

    // ========================
    //  controllers — Status state machine
    // ========================

    #[test]
    fn status_initial_state_is_stopped() {
        let ctx = mk_ctx();
        let ps = ctx.status().get_playback_status();
        assert_eq!(ps.state, State::Stopped);
    }

    #[test]
    fn status_transition_playing() {
        let ctx = mk_ctx();
        ctx.status().update_state(State::Playing);
        assert_eq!(ctx.status().get_playback_status().state, State::Playing);
    }

    #[test]
    fn status_transition_paused() {
        let ctx = mk_ctx();
        ctx.status().update_state(State::Playing);
        ctx.status().update_state(State::Paused);
        assert_eq!(ctx.status().get_playback_status().state, State::Paused);
    }

    #[test]
    fn status_stopped_resets_metadata() {
        let ctx = mk_ctx();
        ctx.status().update_state(State::Playing);
        ctx.status().update_state(State::Stopped);
        let ps = ctx.status().get_playback_status();
        assert_eq!(ps.state, State::Stopped);
        assert!(ps.metadata.is_none());
        assert!(ps.file_name.is_none());
        assert_eq!(ps.index_in_playlist, 0);
    }

    #[test]
    fn status_stopped_resets_position() {
        let ctx = mk_ctx();
        ctx.status().update_position(42);
        ctx.status().update_state(State::Stopped);
        assert_eq!(ctx.status().get_playback_position(), 0);
    }

    #[test]
    fn status_position_updates() {
        let ctx = mk_ctx();
        ctx.status().update_position(10);
        assert_eq!(ctx.status().get_playback_position(), 10);
        ctx.status().update_position(255);
        assert_eq!(ctx.status().get_playback_position(), 255);
    }

    #[test]
    fn status_playing_does_not_reset_position() {
        let ctx = mk_ctx();
        ctx.status().update_position(99);
        ctx.status().update_state(State::Playing);
        assert_eq!(ctx.status().get_playback_position(), 99);
    }

    // ========================
    //  controllers — PlaybackContext
    // ========================

    #[test]
    fn playback_context_volume_default() {
        let ctx = mk_ctx();
        assert_eq!(ctx.volume().load(Ordering::SeqCst), 8);
    }

    #[test]
    fn playback_context_volume_can_be_updated() {
        let ctx = mk_ctx();
        ctx.volume().store(15, Ordering::SeqCst);
        assert_eq!(ctx.volume().load(Ordering::SeqCst), 15);
    }

    // ========================
    //  controllers — PlaybackHandle command dispatch
    // ========================

    #[test]
    fn playback_handle_sends_stop() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        let rx = ch.receiver();

        embassy_futures::block_on(handle.stop());

        match embassy_futures::block_on(rx.receive()) {
            PlayerCommand::Stop => {}
            _ => defmt::panic!("expected Stop"),
        }
    }

    #[test]
    fn playback_handle_sends_pause() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        let rx = ch.receiver();

        embassy_futures::block_on(handle.pause());

        match embassy_futures::block_on(rx.receive()) {
            PlayerCommand::Pause => {}
            _ => defmt::panic!("expected Pause"),
        }
    }

    #[test]
    fn playback_handle_sends_volume_up() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        let rx = ch.receiver();

        embassy_futures::block_on(handle.volume_up());

        match embassy_futures::block_on(rx.receive()) {
            PlayerCommand::VolumeUp => {}
            _ => defmt::panic!("expected VolumeUp"),
        }
    }

    #[test]
    fn playback_handle_sends_volume_down() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        let rx = ch.receiver();

        embassy_futures::block_on(handle.volume_down());

        match embassy_futures::block_on(rx.receive()) {
            PlayerCommand::VolumeDown => {}
            _ => defmt::panic!("expected VolumeDown"),
        }
    }

    #[test]
    fn playback_handle_sends_set_volume() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        let rx = ch.receiver();

        embassy_futures::block_on(handle.set_volume(12));

        match embassy_futures::block_on(rx.receive()) {
            PlayerCommand::SetVolume(12) => {}
            _ => defmt::panic!("expected SetVolume(12)"),
        }
    }

    #[test]
    fn playback_handle_sends_skip_next() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        let rx = ch.receiver();

        embassy_futures::block_on(handle.skip_next());

        match embassy_futures::block_on(rx.receive()) {
            PlayerCommand::Skip(Skip::Next) => {}
            _ => defmt::panic!("expected Skip(Next)"),
        }
    }

    #[test]
    fn playback_handle_sends_skip_previous() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        let rx = ch.receiver();

        embassy_futures::block_on(handle.skip_previous());

        match embassy_futures::block_on(rx.receive()) {
            PlayerCommand::Skip(Skip::Previous) => {}
            _ => defmt::panic!("expected Skip(Previous)"),
        }
    }

    #[test]
    fn playback_handle_get_volume() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        assert_eq!(handle.get_volume(), 8);
        ctx.volume().store(3, Ordering::SeqCst);
        assert_eq!(handle.get_volume(), 3);
    }

    #[test]
    fn playback_handle_status() {
        let ctx = mk_ctx();
        let ch = mk_chan();
        let handle = PlaybackHandle::new(ch.sender(), ctx);
        assert_eq!(handle.status().get_playback_status().state, State::Stopped);
    }

    // ========================
    //  entities — AudioFile / Playlist construction
    // ========================

    #[test]
    fn audio_file_new_and_name() {
        let name: heapless::String<8> = "MYFILE".try_into().unwrap();
        let af = AudioFile::new(name);
        assert_eq!(af.name(), "MYFILE");
    }

    #[test]
    fn playlist_new_returns_name() {
        let name: heapless::String<8> = "TEST".try_into().unwrap();
        let pl = Playlist::new(name.clone(), alloc::vec![]);
        assert_eq!(pl.name(), "TEST");
    }

    #[test]
    fn playlist_with_multiple_files() {
        let name: heapless::String<8> = "TEST".try_into().unwrap();
        let f1 = AudioFile::new("FILE1".try_into().unwrap());
        let f2 = AudioFile::new("FILE2".try_into().unwrap());
        let pl = Playlist::new(name, alloc::vec![f1, f2]);
        assert_eq!(pl.files.len(), 2);
        assert_eq!(pl.files[0].name(), "FILE1");
        assert_eq!(pl.files[1].name(), "FILE2");
    }
}
