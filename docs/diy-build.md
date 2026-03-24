# DIY Build Instructions - Prototype Board

Build your own **PhoniESP32** jukebox using breakout boards! This guide walks
you through assembling the prototype on a breadboard or prototype board.

> **Note**: These instructions are for the breadboard/prototype board version.
> For a more compact on robust solution, the PCB build will be coming soon.

---

## Parts List

All components are available on Aliexpress for under **25 CHF** total.

| Component                        | Quantity | Aliexpress Link                                                       |
| -------------------------------- | -------- | --------------------------------------------------------------------- |
| ESP32-C6 DevKit                  | 1        | [esp-devkit-c1](https://aliexpress.com/item/1005008777558392.html)    |
| TP4056 (Li-ion charger)          | 1        | [32621399438](https://aliexpress.com/item/32621399438.html)           |
| MT3608 (5V boost converter)      | 1        | [1005010193381334](https://aliexpress.com/item/1005010193381334.html) |
| 18650 LiPo cell (3.7V, 600mAh+)  | 1        | [1005009121905266](https://aliexpress.com/item/1005009121905266.html) |
| MAX98357A (I2S DAC + amplifier)  | 1        | [1005010587934639](https://aliexpress.com/item/1005010587934639.html) |
| SD card adapter (SPI)            | 1        | [1005008870179318](https://aliexpress.com/item/1005008870179318.html) |
| MFRC522 (RC522 clone) + S50 tags | 1        | [1005006264253080](https://aliexpress.com/item/1005006264253080.html) |
| 4 Ohm 3W speaker                 | 1        | [1005010804698010](https://aliexpress.com/item/1005010804698010.html) |
| SD card                          | 1        | -                                                                     |
| Breadboard or prototype board    | 1        | -                                                                     |
| Jumper wires                     | ~20      | -                                                                     |

**Optional**:

- RFID keychain figures: [Amazon DE](https://www.amazon.de/dp/B097P3QPVK)
- RFID tags (S50): [Amazon DE](https://www.amazon.de/-/en/dp/B01IH7Y9CM)

---

## Wiring Diagram

![Breakout Boards Wiring Diagram](./PhoniESP32-breakout-boards.svg)

---

## Assembly Steps

### 1. Power Circuit

1. **Connect battery to TP4056**:
   - `B+` → Battery positive (typically red wire)
   - `B-` → Battery negative (typically black wire)

2. **Connect MT3608 boost converter**:
   - `IN+` → TP4056 `OUT+`
   - `IN-` → TP4056 `OUT-`
   - Adjust the potentiometer to output **5V**

3. **Power distribution**:
   - Connect 5V from MT3608 to ESP32 `VIN` (or 5V pin)
   - Connect GND to all components

### 2. ESP32-C6 Connections

| ESP32-C6 Pin | Connected To                              |
| ------------ | ----------------------------------------- |
| `3V3`        | MAX98357 VCC, MFRC522 VCC, SD adapter VCC |
| `GND`        | Common ground                             |
| `GPIO0`      | Button: Pause                             |
| `GPIO1`      | Button: Next/Prev                         |
| `GPIO2`      | Button: Volume up                         |
| `GPIO3`      | Button: Volume down                       |
| `GPIO4`      | USB power detection via voltage divider   |
| `GPIO5`      | SPI `MISO`                                |
| `GPIO6`      | SPI `MOSI`                                |
| `GPIO7`      | SPI `SCK`                                 |
| `GPIO10`     | SD adapter `CS`                           |
| `GPIO15`     | I2S `WS`                                  |
| `GPIO18`     | MFRC522 `SDA`                             |
| `GPIO19`     | MFRC522 `IRQ`                             |
| `GPIO22`     | I2S `DOUT`                                |
| `GPIO23`     | I2S `BCLK`                                |

### 3. MFRC522 RFID Module

The RC522 module uses SPI interface:

- `SDA` → GPIO18
- `SCK` → GPIO7
- `MOSI` → GPIO6
- `MISO` → GPIO5
- `RST` → X
- `GND` → GND
- `IRQ` → GPIO19
- `3.3V` → 3.3V (from ESP32)

### 4. SD Card Adapter

SD card adapter connections:

- `CS` → GPIO10
- `MOSI` → GPIO6
- `SCK` → GPIO7
- `MISO` → GPIO5
- `VCC` → 3.3V (from ESP32)
- `GND` → GND

### 5. MAX98357A I2S DAC

- `LRC` → GPIO15
- `BCLK` → GPIO23
- `DIN` → GPIO22
- `GAIN` → GND
- `SD` → 3.3V (from ESP32)
- `GND` → GND
- `Vin` → 3.3V (from ESP32)

Connect to speaker:

- `+` → Speaker positive
- `-` → Speaker negative

---

## Flash Firmware

1. Download the latest firmware:
   ```bash
   # Download from releases
   wget https://github.com/butzist/phoniesp32/releases/download/v1.0.0/firmware.zip
   unzip firmware.zip
   ```

   Or build from source:
   ```bash
   git clone https://github.com/butzist/phoniesp32.git
   cd phoniesp32
   nix develop
   just build
   ```

2. Install `espflash` if not already installed:
   ```bash
   # Download from GitHub releases
   # or install via cargo: cargo install espflash
   ```

3. Flash the firmware:
   ```bash
   espflash flash --chip esp32c6 firmware
   ```

4. **Important**: Connect the device to a power source (USB or charger). The
   device will not start the wireless AP unless powered.

---

## Prepare SD Card

1. Format the entire SD card as **FAT32** (no partition table)
   ```bash
   # Linux/macOS
   mkfs.vfat /dev/sdX  # Replace X with your device letter

   # Windows: Use SD Card Formatter or diskpart
   ```

2. Insert the SD card into the adapter.

---

## First Boot

1. Power on the device (must be plugged into charger/power)
2. The device creates a Wi-Fi access point:
   - **SSID**: `phoniesp32`
   - **Password**: `12345678`
3. Connect to the AP
4. Open a browser and navigate to:
   - [http://phoniesp32](http://phoniesp32) or
   - [http://phoniesp32.local](http://phoniesp32.local)

5. On the **Settings** page, enter your Wi-Fi credentials
6. Reboot the device - it will now connect to your Wi-Fi when powered

---

## Usage

1. **Upload music**: Navigate to the web interface and transcode/upload audio
   files
2. **Scan RFID tags**: Assign songs/playlists to RFID tags
3. **Playback control**: Use the web UI or physical buttons (if wired)

---

## Troubleshooting

| Issue               | Solution                                                                                              |
| ------------------- | ----------------------------------------------------------------------------------------------------- |
| No AP appears       | Ensure device is powered (plugged into charger)                                                       |
| Can't connect to AP | Check SSID/password, try forgetting network first                                                     |
| Not booting         | Ensure SD card is formatted correctly                                                                 |
| No audio            | Check speaker connections                                                                             |
| RFID not working    | Verify SPI connections, ensure MFRC522 is properly seated. Ensure no conductive plane above or below. |

---

## Next Steps

- [ ] Design and 3D print a case
- [ ] Consider the custom PCB versio for a more compact build

---

_Build cost: ~25 CHF | Runtime: Multiple hours on battery_
