# lamzuctl

A command-line tool and Stream Deck plugin for controlling Lamzu gaming mice on Windows.

## Disclaimer

> **This is unofficial, community-developed software.**
>
> - This project is **not affiliated with, endorsed by, or associated with Lamzu** in any way
> - This software interacts with your mouse hardware through reverse-engineered HID protocols
> - **Use at your own risk** — while care has been taken to ensure safety, unintended bugs could potentially affect your device
> - The authors are **not liable for any damages** resulting from the use of this software
> - This project was developed with AI assistance (Claude)
>
> By using this software, you acknowledge and accept these terms.

## Features

### Command-Line Tool (`lamzuctl`)
- View and switch between profiles (1-5)
- View and change DPI stages
- Configure DPI values per stage
- Monitor battery level and charging status
- List connected Lamzu devices

### Stream Deck Plugin
- **Profile Cycle** — Cycle through selected profiles with a button press
- **DPI Cycle** — Cycle through all DPI stages for the current profile
- **Battery Monitor** — Display battery percentage and charging status
- **Battery Bar** — Optional visual battery indicator as button background

## Supported Devices

Currently tested with:
- Lamzu Maya X (with 8K dongle)

Other Lamzu mice may work but are untested.

## Installation

### Stream Deck Plugin

1. Download the latest `.streamDeckPlugin` file from [Releases](https://github.com/DenzoNL/lamzuctl/releases)
2. Double-click to install via Stream Deck
3. Find "Lamzu Mouse Control" in the Stream Deck action list

### Command-Line Tool

Download the pre-built binary from [Releases](https://github.com/DenzoNL/lamzuctl/releases) or build from source.

## Usage

### CLI Examples

```bash
# List connected devices
lamzuctl list

# Show device info (profile, polling rate, DPI, battery, sensor settings)
lamzuctl info

# List all profiles
lamzuctl profiles

# Switch to profile 2
lamzuctl set profile 2

# Set DPI stage 3 on the current profile
lamzuctl set dpi 3

# Switch to whichever stage is set to 1600 DPI
lamzuctl set dpi 1600

# Show battery level
lamzuctl get battery

# Target a specific device (index from `list`, name substring, or PID)
lamzuctl --device dongle info

# Check whether the mouse is awake (see exit codes below)
lamzuctl get status
```

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (no device, invalid argument, HID failure, ...) |
| 2 | Mouse not responding — asleep, powered off, or out of range. The dongle still answers in this state but reports zeros for every mouse-sourced value, so it is reported separately instead of being shown as "Battery: 0%". |

### Stream Deck

1. Drag "Mouse Control" action to a button
2. Select mode: Profile Cycle, DPI Cycle, or Battery Monitor
3. For Profile Cycle: Check the profiles you want to cycle through
4. Optional: Enable "Show battery as background bar" for visual battery indicator

## Building from Source

### Requirements

- Rust 1.89+
- Windows 10+

### Build

```bash
# CLI only
cargo build --release

# Stream Deck plugin
cargo build --release --features streamdeck --bin lamzu-streamdeck

# Package Stream Deck plugin (release build; pass -Debug for a debug build)
.\build-plugin.ps1

# Run tests
cargo test --all-features
```

The packaged plugin will be created as `io.github.denzonl.lamzuctl.streamDeckPlugin`.

## Troubleshooting the Stream Deck plugin

The plugin logs to `%TEMP%\lamzu-streamdeck.log` (rotated to `.log.old` at 1 MB).

- **Buttons show "asleep"**: the receiver is connected but the mouse is asleep, off, or out of
  range. Move the mouse; the buttons update on the next poll (within 15 seconds) or on a key press.
- **Buttons show "no mouse"**: no Lamzu receiver/cable was found, or it couldn't be opened.

## Technical Notes

- The HID protocol was reverse-engineered for interoperability purposes
- The plugin polls device state every 15 seconds when actions are visible
- Device communication uses HID feature reports on the control interface
- A wireless receiver answers requests even when the mouse is asleep, but with all-zero data;
  the library reports this as a `MouseNotResponding` error

## Reporting Issues

Please report bugs and issues via [GitHub Issues](https://github.com/DenzoNL/lamzuctl/issues).

For security vulnerabilities, please open a GitHub issue or contact the maintainer directly.

## License

[MIT](LICENSE)

---

*This is a hobby project with no guaranteed support or updates.*
