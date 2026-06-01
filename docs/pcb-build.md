# DIY Build Instructions - Custom PCB (Second Prototype)

Build your own **PhoniESP32** jukebox with a custom PCB! This guide walks you
through assembling the compact PCB-based version.

> **Note**: These instructions are for the custom PCB version. For the
> breadboard/prototype board version, see `diy-build.md`.

---

## Parts List

### PCB + SMD Components

The PCB and all SMD components can be ordered from JLCPCB/PCBWay/DigiKey/etc (I
ordered from JLCPCB for ca. **20 CHF** per piece for 5 pieces including
components and assembly). The KiCad project is available in the `pcb/`
directory. In theory you can also just order a PCB and hand solder all
components.

| Component                      | Quantity | Notes                   |
| ------------------------------ | -------- | ----------------------- |
| PhoniESP32 PCB                 | 1        | KiCad project in `pcb/` |
| ESP32-C6-MINI-1U-H4            | 1        |                         |
| BQ24073RGTR (battery charger)  | 1        |                         |
| TPS631000DRLR (3.3V regulator) | 1        |                         |
| MAX98357AETE+T (I2S DAC)       | 1        |                         |
| USBLC6-2SC6 (ESD protection)   | 1        |                         |
| DFE252012P-1R0M (inductor)     | 1        |                         |
| SK12D07VG5 (power switch)      | 1        |                         |
| TF-CARD H1.8 (microSD slot)    | 1        |                         |
| USB Type-C connector           | 1        |                         |
| Resistors & capacitors         | ~20      |                         |
| SPH0645LM4H (microphone)       | 1        | Optional                |

### Components You Need to Source

| Component                         | Quantity | Example Link                                                    |
| --------------------------------- | -------- | --------------------------------------------------------------- |
| 18650 LiPo cell (3.7V, 3000mAh)   | 1        | [Aliexpress](https://aliexpress.com/item/1005009240348340.html) |
| MFRC522 (RC522 clone) + S50 tags  | 1        | [Aliexpress](https://aliexpress.com/item/1005006264253080.html) |
| 4 Ohm 3W full-range speaker       | 1        | [Aliexpress](https://aliexpress.com/item/1005008626624201.html) |
| External 2.4GHz antenna (IPX-3)   | 1        | [Aliexpress](https://aliexpress.com/item/1005005890848674.html) |
| Tactile Push Buttons              | 4        | [Aliexpress](https://aliexpress.com/item/1005011897486277.html) |
| SD card                           | 1        | -                                                               |
| Pin header (2.54mm, 8P) for J4    | 1        | Optional - for RC522                                            |
| Pin header (2.54mm, 5P) for J3    | 1        | Optional - for buttons                                          |
| Pin header (2.54mm, 2P) for J2    | 1        | Optional - for battery connector                                |
| Screw terminal (3.5mm, 2P) for J5 | 1        | Optional - for speaker                                          |
| Dupont cables (female-female)     | 5        | For connecting buttons                                          |
| M2.5 screws and brass inserts     | 6 each   | For mounting in case                                            |

**Optional**:

- RFID keychain figures: [Amazon DE](https://www.amazon.de/dp/B097P3QPVK)
- RFID tags (S50): [Amazon DE](https://www.amazon.de/-/en/dp/B01IH7Y9CM)

---

## Connector Overview

The PCB has populated and unpopulated connector footprints:

| Ref | Type                      | Pins | Purpose      | Populated? |
| --- | ------------------------- | ---- | ------------ | ---------- |
| J1  | USB Type-C (SMD)          | 16   | Power & data | Yes        |
| J2  | Pin header, 2.54mm        | 2    | Battery      | DNP        |
| J3  | Pin header, 2.54mm        | 5    | Buttons      | DNP        |
| J4  | Pin header/socket, 2.54mm | 8    | RC522        | DNP        |
| J5  | Screw terminal, 3.5mm     | 2    | Speaker      | DNP        |

### J2 Pinout (2-pin, 2.54mm) - Battery

| Pin | Signal | Connected To             |
| --- | ------ | ------------------------ |
| 1   | BAT+   | Battery positive (red)   |
| 2   | BAT-   | Battery negative (black) |

### J3 Pinout (5-pin, 2.54mm) - Buttons

| Pin | Signal   | Connected To            |
| --- | -------- | ----------------------- |
| 1   | GND      | One side of all buttons |
| 2   | Button 1 | Pause button            |
| 3   | Button 2 | Next/Prev button        |
| 4   | Button 3 | Volume down button      |
| 5   | Button 4 | Volume up button        |

### J4 Pinout (8-pin, 2.54mm) - RC522

| Pin | Signal | Connected To   |
| --- | ------ | -------------- |
| 1   | SDA/CS | RC522 `SDA`    |
| 2   | SCK    | RC522 `SCK`    |
| 3   | MOSI   | RC522 `MOSI`   |
| 4   | MISO   | RC522 `MISO`   |
| 5   | IRQ    | RC522 `IRQ`    |
| 6   | GND    | RC522 `GND`    |
| 7   | EN     | RC522 `ENABLE` |
| 8   | 3.3V   | RC522 `VCC`    |

---

## Assembly Steps

### 1. Ordering the PCB

1. Open the KiCad project in `pcb/PhoniESP32/`
2. Generate Gerber files (File → Fabrication → Gerbers or with manufacturing
   plugin)
3. Upload to JLCPCB/PCBWay/DigiKey/etc and order PCB
4. Optional: Select the SMT assembly service with the components listed above

![Photo of PCB top](docs/pcb-top.jpg)

### 2. Solder Connectors (J2, J3, J4, J5)

These are marked DNP (Do Not Populate) by default. Solder them based on your
preferred wiring method:

**J2 (2-pin, battery)**:

- Solder a JST PH socket, or
- If your battery doesn't have a JST PH connector, solder directly, or use a
  screw terminal

**J3 (5-pin, buttons)**:

- Solder a 5-pin male pin header
- Buttons are connected with female-to-male DuPont cables to each pin

**J4 (8-pin, RC522)**:

- Solder an 8-pin female pin socket (for easy plugging of RC522) or male
  pin/JST/Molex header if you want to mount the RC522 on the lid, like I did.

**J5 (2-pin, speaker screw terminal)**:

- Solder the 2-pin screw terminal for easy speaker connection

![Photo of connectors soldered](docs/pcb-connectors.jpg)

### 3. Attach Heat Inserts to Case

The case design uses M2.5 brass heat inserts. Instructions for installing them:

![Photo of heat inserts](docs/heat-inserts.jpg)

1. Print the case parts from `case/` directory:
   - `PCBCase-Top.stl`
   - `PCBCase-Bottom.stl`
   - `PCBCase-ButtonHolder.stl`
2. Heat up a soldering iron to ~200°C
3. Place the heat insert on the designated hole in the printed part
4. Press down gently with the soldering iron tip until the insert is flush with
   the surface
5. Repeat for all mounting holes

### 4. Mount PCB and RC522 in Case

![Photo of PCB mounted in case](docs/pcb-in-case.jpg)

1. The design has mounting pins that can be melted over the modules with a
   soldering iron for a secure fit
2. Secure the PCB to the case bottom by melting the mounting pins
3. Mount the RC522 module under the case top in the same way

![Photo of RC522 mounted in case top](docs/rc522-mounted.jpg)

### 5. Connect RC522 RFID Module

1. Wire the RC522 module to J4 using the pinout table above
2. A female-female DuPont cable or custom harness works well
3. Keep the connections short and tidy.

![Photo of RC522 wiring](docs/rc522-wiring.jpg)

### 6. Connect Speaker

- The speaker connects to J5 (screw terminal)
- Mount the speaker on the side of the case using the designated mounting
  points.

![Photo of speaker wiring](docs/speaker-wiring.jpg)

### 7. Connect Buttons

Each button connects between GND and the corresponding signal pin on J3. All
buttons share a common GND.

| Button       | J3 Pin |
| ------------ | ------ |
| GND (common) | 1      |
| Pause        | 2      |
| Next/Prev    | 3      |
| Volume +     | 4      |
| Volume -     | 5      |

Solder one side of all buttons to a common GND wire, then connect the other side
of each button to its respective J3 pin using individual wires.

![Photo of button wiring](docs/buttons-wiring.jpg)

### 8. Connect Battery

If the battery has a JST PH 2.0mm connector, it plugs directly into J2. If not,
wire it to the 2-pin header:

- Battery `+` (red) → J2 pin 1
- Battery `-` (black) → J2 pin 2

**Warning**: Double-check polarity before connecting!

![Photo of battery wiring](docs/battery-wiring.jpg)

### 9. Connect External Antenna

The ESP32-C6-MINI-1U module has an IPX connector for an external antenna.
Connect the IPX-3 PCB antenna to the module.

![Photo of antenna connection](docs/antenna-wiring.jpg)

### 10. Final Assembly

1. Place the battery in the case
2. Route all wires neatly to avoid pinching
3. Close the case and secure with screws
4. Insert the FAT32-formatted SD card

![Photo of final assembly](docs/final-assembly.jpg)

---

## Flash Firmware

1. Download the latest firmware:
   ```bash
   wget https://github.com/butzist/phoniesp32/releases/download/v1.1.0/firmware.zip
   unzip firmware.zip
   ```

   Or build from source:
   ```bash
   git clone https://github.com/butzist/phoniesp32.git
   cd phoniesp32
   nix develop
   cd firmware && just build
   ```

2. Install `espflash` (download from
   [GitHub releases](https://github.com/esp-rs/espflash/releases/tag/v4.3.0) or
   via package manager)

3. Flash the firmware:
   ```bash
   espflash flash --chip esp32c6 firmware
   ```

4. **Important**: Connect the device to a power source (USB or charger). The
   device will not start the wireless AP unless powered.

---

## Prepare SD Card

If not already formatted, format the SD card as **FAT32** with an MBR partition
table.

### Linux

```bash
mkfs.vfat /dev/sdX1
```

### macOS

```bash
sudo diskutil eraseDisk FAT32 PHONIESP32 MBRFormat /dev/disk2
```

### Windows

Use DiskPart or FAT32 Format (guiformat.exe) for cards >32GB.

Insert the SD card into the PCB's microSD slot.

---

## First Boot

1. Power on the device (plug into USB or flip the power switch)
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
3. **Playback control**: Use the web UI, RFID scanning, or physical buttons

---

## Troubleshooting

| Issue               | Solution                                                                                                     |
| ------------------- | ------------------------------------------------------------------------------------------------------------ |
| No AP appears       | Ensure device is powered (USB or battery with switch on)                                                     |
| Can't connect to AP | Check SSID/password, device will fall back to starting as AP                                                 |
| Not booting         | Ensure SD card is formatted correctly                                                                        |
| No audio            | Check speaker connections to J5                                                                              |
| RFID not working    | Verify J4 connections, ensure RC522 is properly seated and not obstructed by close-by conductive material    |
| Poor WiFi range     | Ensure external antenna is connected to the IPX connector and not obstructed by close-by conductive material |

---

## Access Logs

```bash
cd firmware
just run       # Flash and monitor
# or
just monitor   # Just monitor (if already flashed)
```

Or using espflash directly:

```bash
espflash monitor --chip esp32c6 --log-format defmt --elf firmware/target/riscv32imac-unknown-none-elf/release/firmware
```

---

_Build cost: ~30-35 CHF | Runtime: Multiple days on 3000mAh battery_
