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

- [x] Rust/Embassy project scaffold
- [ ] File upload & transcoding
- [ ] Web server & UI (Dioxus)
- [ ] Audio pipeline (I2S → MAX98357)
- [ ] Playback control
- [ ] SD card access
- [ ] RFID tag → playlist mapping
- [ ] Power management & optimizations
- [ ] Wiring diagrams & build guide

---

## 🤝 Contributing

Want to help build the cheapest, most fun, and hackable jukebox for kids?

- Share wiring diagrams
- Improve the Rust firmware
- Extend the web UI
- Optimize power usage

PRs, issues, and ideas are always welcome!

---

## 📜 License

MIT License — free to use, hack, and share.
