# phoniesp32 🎶🔋

A **kid-friendly jukebox** powered by the **ESP32**, inspired by
[Phoniebox](https://phoniebox.de). Unlike the original Raspberry Pi–based
version, **phoniesp32** is designed to be **battery-operated, low-power, and
ultra-portable**.

> ⚠️ Work in Progress — Contributions, ideas, and wiring tips are very welcome!

---

## ✨ Features

- **ESP32-based jukebox**: replaces the Raspberry Pi with an ESP-WROOM-32, or
  newer.
- **Low-power & battery-ready**: runs on a Li-ion cell with TP4056 charging and
  MT3608 boost converter.
- **Bare-metal Rust firmware**: built on the [`embassy`](https://embassy.dev/)
  async embedded framework.
- **Rust-powered Web UI**: implemented with [Dioxus](https://dioxuslabs.com/)
  and served directly from the ESP32.
- **Audio transcoding & upload**: convert music files via the web frontend to
  IMA ADPCM, upload to device, and play back instantly.
- **RFID support**: scan S50 fobs with the FRC522 module to trigger playlists.
- **Affordable**: target build cost **under 15 CHF**.

---

## 🛠 Components

- **MCU**: ESP-WROOM-32
- **Charging/Power**: TP4056 (Li-ion charger), MT3608 (boost converter)
- **Battery**: Li-ion cell (e.g. 18650)
- **Audio**: MAX98357 (I2S DAC + amplifier)
- **Storage**: SD card adapter
- **RFID**: FRC522 module + S50 RFID fobs
- **Misc**: wiring TBD (PRs welcome!)

---

## 🚧 Project Status

![Screenshot of current prototype](./docs/progress-1.jpeg) Current
work-in-progress prototype with lots of noisy data lines...

- [ ] Hardware
  - [x] Prototype on breadboard
  - [ ] Soldered Prototype
  - [ ] Battery charger
  - [ ] Schematics
  - [ ] Custom PCB
  - [ ] Design and 3D print case
- [x] UI (Dioxus)
  - [x] File transcoding
  - [ ] Served from device
  - [ ] Show last scanned RFID tag ID
  - [ ] Switch to Bluma CSS
  - [ ] Playback control
  - [ ] Associate file with RFID tag
  - [ ] Configure Wi-Fi settings
  - [ ] Playlists
  - [ ] List known tags
  - [ ] List uploaded files
  - [ ] Speed up transcoding
- [ ] Firmware (Embassy)
  - [x] Fallback to Wi-Fi AP mode
  - [x] Audio pipeline (I2S → MAX98357)
  - [x] SD card access
  - [x] Playback control via buttons
  - [ ] Playback control via RFID tags
  - [ ] Pause playback
  - [ ] Turn off Wi-Fi when on battery power
  - [ ] Playlists
  - [ ] Power management
  - [ ] SD card partitioning support
  - [ ] Async / IRQ for RFID reader
  - [ ] Playback control via BLE
- [ ] Web API (picoserve)
  - [x] Associate file with RFID tag
  - [x] Playback control
  - [ ] Show playback status
  - [ ] Show last scanned RFID tag ID
  - [x] Configure Wi-Fi settings
  - [ ] List known tags
  - [ ] List uploaded files
- [ ] Build system
  - [ ] Build and bundle all components
  - [ ] Build via GitHub actions
  - [ ] Build instructions
- [ ] Build guide

---

## 🤝 Contributing

Want to help build the cheapest, most fun, and hackable jukebox for kids?

- Share schematics
- Improve the Rust firmware
- Extend the web UI
- Optimize power usage

PRs, issues, and ideas are always welcome!

---

## 📜 License

MIT License — free to use, hack, and share.
