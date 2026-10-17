# 🦀🍇 RustBerry-PoE-Monitor

[![Build](https://github.com/mkiiim/RustBerry-PoE-Monitor-mkiiim/actions/workflows/build.yaml/badge.svg)](https://github.com/mkiiim/RustBerry-PoE-Monitor-mkiiim/actions/workflows/build.yaml)
![Rust](https://img.shields.io/badge/Rust-%23000000.svg?logo=rust&logoColor=white)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
![Raspi](https://img.shields.io/badge/Raspberry%20Pi-A22846?logo=Raspberry%20Pi&logoColor=white)

RustBerry-PoE-Monitor is a Rust-based monitoring and control tool for the Raspberry Pi, specifically designed for use with the [Waveshare PoE HAT (B)](https://www.waveshare.com/wiki/PoE_HAT_(B)). 

![Example image](/docs/IMG_3890.webp)

This is a fork of [jackra1n/RustBerry-PoE-Monitor](https://github.com/jackra1n/RustBerry-PoE-Monitor). It adds a screen-off mode with a wake endpoint, a reading-order layout format and stricter config checking, described below.

This tool provides real-time monitoring of your Raspberry Pi's system statistics, including network interface(s), IP address(es), memory usage, CPU load, and more, displayed on the PoE HAT's OLED display. Additionally, it offers fan control to maintain optimal operating temperatures.

## 📖 Table of Contents

- [🦀🍇 RustBerry-PoE-Monitor](#-rustberry-poe-monitor)
  - [📖 Table of Contents](#-table-of-contents)
  - [🌟 Features](#-features)
  - [📦 Installation](#-installation)
    - [Building on the Pi](#building-on-the-pi)
    - [Installing](#installing)
    - [Updating](#updating)
  - [📝 Configuration](#-configuration)
    - [Fan Control Settings](#fan-control-settings)
    - [Screen Off](#screen-off)
    - [Wake Endpoint](#wake-endpoint)
      - [Homebridge Switch](#homebridge-switch)
    - [Display Configuration](#display-configuration)
      - [Layout File](#layout-file)
      - [Lines](#lines)
      - [Rows](#rows)
      - [Templates](#templates)
      - [Available Fonts](#available-fonts)
      - [Example](#example)
      - [Upgrading from the old format](#upgrading-from-the-old-format)
      - [Checking a Configuration](#checking-a-configuration)
      - [Previewing a Layout](#previewing-a-layout)
  - [🛠️ Building](#️-building)
    - [Prerequisites](#prerequisites)
    - [Building for Raspberry Pi](#building-for-raspberry-pi)
  - [🏃‍♂️ Running](#️-running)
  - [Links](#links)

## 🌟 Features

- **Minimal** resource usage
- Developed in Rust for memory safety 🦀
- Display real-time system statistics on the PoE HAT's OLED screen 📊
  - network interface(s) - will rotate through all active `eth` and `wlan` interfaces
  - IP address(es) - will rotate through corresponding adddresses for each interface
  - CPU usage
  - CPU temp
  - memory usage
  - disk usage
- Configurable display orientation in both landscape and portrait 
- Customizable display layout
- Automatic fan control based on the CPU temperature 🌡️
- Screen-off mode to prevent OLED burn-in, with an HTTP endpoint to wake the display on demand (e.g. from a Homebridge switch)

## 📦 Installation

This fork has no prebuilt releases, so build it on the Raspberry Pi it will run on.

### Building on the Pi

Enable I2C, install Rust, then build:

```bash
sudo raspi-config nonint do_i2c 0
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/mkiiim/RustBerry-PoE-Monitor-mkiiim.git
cd RustBerry-PoE-Monitor-mkiiim
cargo build --release
```

### Installing

Install the binary, the layouts and the systemd service, check the layout, then start the service:

```bash
sudo install -m 755 target/release/rustberry-poe-monitor /usr/local/bin/
sudo install -d /etc/rustberry-poe-monitor
sudo install -m 644 etc_rustberry-poe-monitor/*.json /etc/rustberry-poe-monitor/
sudo install -m 644 etc_systemd_system/rustberry-poe-monitor.service /etc/systemd/system/
rustberry-poe-monitor --check-config --config /etc/rustberry-poe-monitor/portrait.json
sudo systemctl daemon-reload
sudo systemctl enable --now rustberry-poe-monitor
```

The service runs as the `pi` user, which needs access to I2C (the `i2c` group). Edit `User=` and `Group=` in the service file if your user is different.

### Updating

```bash
git pull
cargo build --release
sudo install -m 755 target/release/rustberry-poe-monitor /usr/local/bin/
rustberry-poe-monitor --check-config --config /etc/rustberry-poe-monitor/portrait.json
sudo systemctl restart rustberry-poe-monitor
```

If the layout format has changed, install the new layouts as well, after copying any you have edited.

## 📝 Configuration

### Fan Control Settings

The fan turns on at 40°C (`temp-on`) and only stops once the CPU cools to 30°C (`temp-off`). On startup it runs unless the CPU is already at or below `temp-off`, and it stays on whenever the CPU temperature can't be read.

Fan control requires the PoE HAT's FAN switch to be set to **P0**. In the **EN** position the fan always runs and these settings have no effect. The switch position can't be read in software, so check it on the board.

To change the fan on/off temperature, you can add `temp-on` and `temp-off` arguments to the application.
If you installed the application using the install script, you can edit the systemd service file to change the arguments.

```bash
sudo nano /etc/systemd/system/rustberry-poe-monitor.service 
```

Change the `ExecStart` line to the following:
```bash
ExecStart=/usr/local/bin/rustberry-poe-monitor --temp-on 60 --temp-off 50
```

### Screen Off

To protect the OLED from burn-in, `--screen-off` keeps the display switched off while fan control continues to run:

```bash
ExecStart=/usr/local/bin/rustberry-poe-monitor --config /etc/rustberry-poe-monitor/portrait.json --screen-off
```

### Wake Endpoint

With `--listen`, a small HTTP endpoint can wake the display for a while, e.g. from a Homebridge switch:

```bash
ExecStart=/usr/local/bin/rustberry-poe-monitor --config /etc/rustberry-poe-monitor/portrait.json --screen-off --listen 0.0.0.0:8590
```

- `/wake` turns the display on for 5 minutes (`--wake-seconds`), or `/wake?seconds=N` (up to 3600)
- `/sleep` turns it off again
- `/status` returns `{"on":true,"remaining":295}`

GET and POST are both accepted. There is no authentication, so only listen on a trusted network.

#### Homebridge Switch

With [homebridge-http-switch](https://github.com/Supereg/homebridge-http-switch), each Pi gets a switch that shows whether its display is awake and turns itself off when the wake time runs out:

```json
{
  "accessory": "HTTP-SWITCH",
  "name": "pidev Screen",
  "switchType": "stateful",
  "onUrl": "http://10.0.20.133:8590/wake",
  "offUrl": "http://10.0.20.133:8590/sleep",
  "statusUrl": "http://10.0.20.133:8590/status",
  "statusPattern": "\"on\":true",
  "pullInterval": 10000
}
```

A stateful switch can only call one URL, so add one switch per Pi. To wake several Pis at once, create a scene in the Home app that turns their switches on.

### Display Configuration

RustBerry-PoE-Monitor uses a JSON file to define the OLED display layout and contents. You can specify a custom configuration file using the `--config` argument in the `/etc/systemd/system/rustberry-poe-monitor.service` file:

```bash
ExecStart=/usr/local/bin/rustberry-poe-monitor --config /path/to/your/config.json --temp-on 60 --temp-off 50
```

or on the command line

```bash
rustberry-poe-monitor --config /path/to/your/config.json --temp-on 60 --temp-off 50
```

The application comes with two pre-configured layouts:
- `landscape.json`: For horizontal display orientation (128×32 pixels)
- `portrait.json`: For vertical display orientation (32×128 pixels)

| Landscape | Portrait |
|---------|---------|
| <img src="docs/IMG_4552 Medium.png" alt="no json file" width="300"> | <img src="docs/IMG_4554 Medium.png" alt="no json file" width="300"> |

The included `/etc/systemd/system/rustberry-poe-monitor.service` uses `portrait.json`, which is also the default without `--config`.

#### Layout File

A layout file describes what is drawn, top to bottom, in reading order. Lines go down the display and rows go across it, so there are no x/y coordinates to work out:

```json
{
  "orientation": "portrait",
  "font": "FONT_6X12",
  "align": "left",
  "leading": 9,
  "page_seconds": 5,
  "lines": [ ... ]
}
```

| Field | Description |
|-------|-------------|
| `orientation` | Required. `"portrait"` (32×128) or `"landscape"` (128×32), as the display is read |
| `font` | Default font. Optional, default `FONT_6X12` |
| `align` | Default alignment: `"left"`, `"center"` or `"right"`. Optional, default `left` |
| `leading` | Default line spacing (see [Lines](#lines)). Optional |
| `page_seconds` | Seconds each page is shown. Optional, default 5; only matters with several pages |
| `lines` | The lines to draw |
| `pages` | Instead of `lines`: a list of pages, each `{ "lines": [...] }` with optional `font`, `align` and `leading` |

A file has either `lines` or `pages`, not both. With several pages the display steps through them, `page_seconds` each. Unknown fields anywhere are errors.

#### Lines

Each entry in a `lines` array is one of:

| Line | Description |
|------|-------------|
| `"CPU {cpu_usage}"` | A text line in the inherited font and alignment |
| `{ "text": "...", "font", "align", "leading", "dy" }` | A text line with its own settings (all optional except `text`) |
| `{ "row": [...], "font", "align", "leading", "dy" }` | A line made of pieces placed side by side (see [Rows](#rows)) |
| `{ "lines": [...], "font", "align", "leading" }` | A group: its settings apply to every line inside it. Not a line itself |
| `{ "space": 2 }` | Vertical gap in pixels. May be negative to move back up |

Each text or row line moves down by its `leading` and puts its text baseline there. `leading` is the line's own, or inherited; if none is set, it is the height of the tallest font in the line. `space` moves down (or up) by its value. `dy` shifts a line up (negative) or down when it is drawn without moving the lines after it.

`align` places a text line, or a whole row, across the display. If centred or right-aligned text is wider than the display it starts at the left edge.

`font`, `align` and `leading` are inherited from the top level to a page, group and line. A row's `font` is also the default for its pieces.

#### Rows

Each entry in a `row` array is one of:

| Piece | Description |
|-------|-------------|
| `"%"` | Text in the inherited font |
| `{ "text": "...", "font", "dy", "width", "align" }` | Text with its own settings (all optional except `text`) |
| `{ "row": [...], "font", "dy", "width", "align" }` | A nested row |
| `{ "space": 2 }` | Horizontal gap in pixels. May be negative to pull the next piece closer |
| `{ "fill": true }` | Flexible gap that takes up the remaining width |

Pieces are placed left to right, each taking its text width.

- **Boxes:** with `width`, a piece takes exactly that many pixels and its contents are aligned inside the box by `align` (default left). This keeps changing values lined up, e.g. right-aligning a reading in a 24px box. Text wider than its box overflows the way it is aligned (right-aligned text extends to the left). `align` without `width` is an error.
- **Fill:** in a row line (full display width) or a boxed nested row (its `width`), the space not used by other pieces is shared between its `fill`s. A row with a `fill` always uses the full width, so its `align` has no effect. In a nested row without `width`, a `fill` is zero.
- **dy:** adds up, so a piece's `dy` is added to its row's and line's.

#### Templates

All text is a template: `{name}` is replaced with a value, `{{` and `}}` give literal braces, and anything else is shown as is.

| Value | Description |
|-------|-------------|
| `{interface}` | Network interface name (e.g., "eth0.99") |
| `{interface_phys}` | Interface name, or its prefix for a VLAN interface (e.g., "eth0", or "eth" for "eth0.99") |
| `{interface_numvlan}` | Interface number and VLAN for a VLAN interface (e.g., "0.99" for "eth0.99"), otherwise empty |
| `{ip}` | IP address (e.g., "192.168.0.1") |
| `{ip_octet0}` - `{ip_octet3}` | Individual octets of the IP address |
| `{cpu_usage}` | CPU usage percentage |
| `{cpu_temp}` | CPU temperature in Celsius |
| `{ram_usage}` | RAM usage percentage |
| `{disk_usage}` | Disk usage percentage |
| `{hostname}` | The machine's hostname, read at startup |

Usage and temperature are shown with one decimal below 100 and without one from 100 up, so they stay four characters wide. A temperature that can't be read is shown as `--`.

#### Available Fonts

The system supports multiple built-in fonts:

- `FONT_5X8`: Small 5×8 pixel font
- `FONT_6X12`: Medium 6×12 pixel font
- `PCSENIOR8_STYLE`: PC Senior font (8px width)
- `PROFONT9`: ProFont 9-point
- `PROFONT12`: ProFont 12-point

#### Example

The shipped `portrait.json`:

```json
{
  "orientation": "portrait",
  "font": "FONT_6X12",
  "lines": [
    { "leading": 8, "font": "PCSENIOR8_STYLE", "lines": ["{interface_phys}", "{interface_numvlan}"] },
    { "space": 2 },
    { "leading": 8, "lines": [
      { "row": [{ "space": 14 }, "{ip_octet0}"] },
      { "row": [{ "space": 8 }, ".{ip_octet1}"] },
      { "row": [{ "space": 8 }, ".{ip_octet2}"] },
      { "row": [{ "space": 8 }, ".{ip_octet3}"] }
    ] },
    { "space": 4 },
    { "leading": 9, "lines": [
      { "text": "CPU", "font": "FONT_5X8", "align": "right" },
      { "row": [{ "text": "{cpu_usage}", "width": 24, "align": "right" }, { "space": 2 }, "%"] },
      { "row": [{ "text": "{cpu_temp}", "width": 24, "align": "right" },
                { "text": "o", "font": "FONT_5X8", "dy": -4 }, { "space": -2 }, { "text": "C", "font": "FONT_5X8" }] }
    ] },
    { "space": 3 },
    { "leading": 9, "lines": [
      { "text": "RAM", "font": "FONT_5X8", "align": "right" },
      { "row": [{ "text": "{ram_usage}", "width": 24, "align": "right" }, { "space": 2 }, "%"] }
    ] },
    { "space": 3 },
    { "leading": 9, "lines": [
      { "text": "DISK", "font": "FONT_5X8", "align": "right" },
      { "row": [{ "text": "{disk_usage}", "width": 24, "align": "right" }, { "space": 2 }, "%"] }
    ] }
  ]
}
```

- **Interface block:** the interface split over two lines, e.g. "eth" and "0.99", 8px apart.
- **IP block:** one octet per line. The first is indented 14px and the others 8px plus a 6px ".", so the numbers line up.
- **CPU, RAM and DISK blocks:** a right-aligned label, then each reading right-aligned in a 24px box followed by its unit, 9px apart, with `space` between blocks. The degree sign is a small raised "o" (`dy: -4`) pulled against the "C" with a negative `space`.

`landscape.json` uses rows the same way: below a centred interface and IP line, each line is split into two 64px halves by putting the left reading in a nested row with `"width": 64`.

#### Upgrading from the old format

Earlier versions used a different format with `elements`, `position` and `components`. It is no longer read: a file with `elements` is rejected with an error pointing here, and the display shows "CONFIG ERROR". The shipped `portrait.json` and `landscape.json` have been converted; a custom layout needs to be rewritten in the format above.

#### Checking a Configuration

The configuration is checked when it is loaded. These are errors rather than being ignored or drawn wrongly:

- an unknown field, font or alignment, or a line or row piece of the wrong shape
- an unknown value in a template, such as `{cpu_tmep}`, an empty `{}`, or an unmatched `{` or `}`
- a character the font can't draw
- a file in the old format

If the file is missing or invalid, the error is logged and the display shows the hostname and "CONFIG ERROR". Fan control keeps running.

To check a file before restarting the service:

```bash
rustberry-poe-monitor --check-config --config /etc/rustberry-poe-monitor/portrait.json
```

It prints `OK` or the problem and where it is (e.g. `lines[3].row[1]`), and exits with status 0 or 1. It also lays out each page with the widest values (e.g. `99.9`, `255.255.255.255`, `eth0.999`) and prints a warning for text that would run off the side, top or bottom of the screen. Warnings don't change the exit status. The hostname is not checked at its worst case, so a long hostname can still run off the screen.

#### Previewing a Layout

To see a layout without waking the display or restarting the service, draw it to a PNG:

```bash
rustberry-poe-monitor --config /etc/rustberry-poe-monitor/portrait.json --render layout.png
```

The image is the display as it is read, scaled up four times, using this machine's current readings. Add `--sample` to use fixed sample values instead, so two renders can be compared. For a layout with several pages, add `--page N` to draw page N (starting at 1, the default).

## 🛠️ Building

### Prerequisites
For building for Rasberry Pi I'm using [cross](https://github.com/cross-rs/cross)

### Building for Raspberry Pi
```bash
 cross build --target=aarch64-unknown-linux-gnu --release
```

## 🏃‍♂️ Running

Just run the binary file
```bash
./rustberry-poe-monitor
```

## Links

- [Waveshare PoE HAT (B)](https://www.waveshare.com/wiki/PoE_HAT_(B))
- [raspi-poe-mon](https://github.com/klamann/raspi-poe-mon) - python implementation and original idea for the display layout
- [PCSenior](https://www.1001fonts.com/pc-senior-font.html) - Font used for the display
- [cross](https://github.com/cross-rs/cross) - Rust tool for building cross-platform binaries