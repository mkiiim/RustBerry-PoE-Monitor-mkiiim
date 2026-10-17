# 🦀🍇 RustBerry-PoE-Monitor

[![Build](https://github.com/jackra1n/RustBerry-PoE-Monitor/actions/workflows/build.yaml/badge.svg)](https://github.com/jackra1n/RustBerry-PoE-Monitor/actions/workflows/build.yaml)
![Rust](https://img.shields.io/badge/Rust-%23000000.svg?logo=rust&logoColor=white)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
![Raspi](https://img.shields.io/badge/Raspberry%20Pi-A22846?logo=Raspberry%20Pi&logoColor=white)

RustBerry-PoE-Monitor is a Rust-based monitoring and control tool for the Raspberry Pi, specifically designed for use with the [Waveshare PoE HAT (B)](https://www.waveshare.com/wiki/PoE_HAT_(B)). 

![Example image](/docs/IMG_3890.webp)

This tool provides real-time monitoring of your Raspberry Pi's system statistics, including network interface(s), IP address(es), memory usage, CPU load, and more, displayed on the PoE HAT's OLED display. Additionally, it offers fan control to maintain optimal operating temperatures.

## 📖 Table of Contents

- [🦀🍇 RustBerry-PoE-Monitor](#-rustberry-poe-monitor)
  - [📖 Table of Contents](#-table-of-contents)
  - [🌟 Features](#-features)
  - [📦 Installation](#-installation)
    - [Easy Installation](#easy-installation)
    - [Manual Installation](#manual-installation)
  - [📝 Configuration](#-configuration)
    - [Fan Control Settings](#fan-control-settings)
    - [Screen Off](#screen-off)
    - [Wake Endpoint](#wake-endpoint)
      - [Homebridge Switch](#homebridge-switch)
    - [Display Configuration](#display-configuration)
      - [JSON Configuration Schema](#json-configuration-schema)
      - [Dynamic Variables](#dynamic-variables)
      - [Available Fonts](#available-fonts)
      - [Examples](#examples)
      - [Checking a Configuration](#checking-a-configuration)
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

### Easy Installation

Run the following command to install:
```bash
curl -sSL https://rustberry.jackra1n.com/install | sudo bash
```

And that's it!

### Manual Installation

Check out the [wiki page](https://github.com/jackra1n/RustBerry-PoE-Monitor/wiki/Manual-Installation) to learn how to install manually.

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

The included `/etc/systemd/system/rustberry-poe-monitor.service` is defaulted to using the `landscape.json`

#### JSON Configuration Schema

The configuration file has this basic structure:

```json
{
  "orientation": "landscape",  // or "portrait"
  "width": 128,
  "height": 32,
  "elements": [
    // Array of display elements
  ]
}
```

Each element represents a piece of information on the display:

```json
{
  "id": "element_name",
  "position": {
    "x": "center",  // or number, or relative positioning
    "y": 10         // position from top in pixels
  },
  "components": [
    // Array of text components
  ]
}
```

`y` is the text baseline in pixels from the top. `x` supports three formats:
- Text values: `"x": "center"` (also `"left"` or `"right"`)
- Numeric values: `"x": 10` (exact pixel position)
- Relative positioning:
  ```json
  "x": {
    "align": "right",  // "left", "center", or "right"
    "anchor": 38       // reference pixel position
  }
  ```

Pixel co-ordinate positions used for `"anchor":` values increase top-to-bottom and left-to-right . (i.e., 0,0 is at the top left corner of the screen, with respect to its `"orientation":`)

Components define the actual content to render:

```json
{
  "value": {
    "text": "cpu_usage",  // Static text or dynamic variable
    "font": "PCSENIOR8_STYLE"
  },
  "prefix": {  // Optional
    "text": "CPU: ",
    "font": "FONT_5X8"
  },
  "suffix": {  // Optional
    "text": "%",
    "font": "FONT_6X12"
  }
}
```

#### Dynamic Variables

You can use these dynamic variables in the `text` field of a `value`. Any other text is shown as is:

| Variable | Description |
|----------|-------------|
| `ip_info.0` | Network interface name (e.g., "eth0.99")|
| `ip_info.1` | IP address (e.g., "192.168.0.1")|
| `interface_phys` | Interface name, or its prefix for a VLAN interface (e.g., "eth0", or "eth" for "eth0.99") |
| `interface_numvlan` | Interface number and VLAN for a VLAN interface (e.g., "0.99" for "eth0.99"), otherwise empty |
| `ip_octets(0)` - `ip_octets(3)` | Individual octets of the IP address |
| `cpu_usage` | CPU usage percentage |
| `cpu_temp` | CPU temperature in Celsius |
| `ram_usage` | RAM usage percentage |
| `disk_usage` | Disk usage percentage |

Usage and temperature are shown with one decimal below 100 and without one from 100 up, so they stay four characters wide. A temperature that can't be read is shown as `--`.

#### Available Fonts

The system supports multiple built-in fonts:

- `FONT_5X8`: Small 5×8 pixel font
- `FONT_6X12`: Medium 6×12 pixel font
- `PCSENIOR8_STYLE`: PC Senior font (8px width)
- `PROFONT9`: ProFont 9-point
- `PROFONT12`: ProFont 12-point

#### Examples

Display CPU usage with percentage sign:

```json
{
  "id": "cpu_usage",
  "position": {
    "x": {
      "align": "right",
      "anchor": 38
    },
    "y": 20
  },
  "components": [
    {
      "value": {
        "text": "cpu_usage",
        "font": "PCSENIOR8_STYLE"
      }
    }
  ]
},
{
  "id": "cpu_usage_units",
  "position": {
    "x": {
      "align": "left",
      "anchor": 40
    },
    "y": 20
  },
  "components": [
    {
      "value": {
        "text": "%",
        "font": "FONT_6X12"
      },
      "suffix": {
        "text": "CPU",
        "font": "FONT_5X8"
      }
    }
  ]
}
```

#### Checking a Configuration

The configuration is checked when it is loaded. An unknown field, font or alignment, or a misspelled variable such as `cpu_tmep`, is an error rather than being ignored or shown as text. Lowercase text with an underscore is treated as a variable name, so it must be one of the variables above.

If the file is missing or invalid, the error is logged and the display shows the hostname and "CONFIG ERROR". Fan control keeps running.

To check a file before restarting the service:

```bash
rustberry-poe-monitor --check-config --config /etc/rustberry-poe-monitor/portrait.json
```

It prints `OK` or the problem with its line and column, and exits with status 0 or 1.

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