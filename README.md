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

# Show current status
lamzuctl status

# Switch to profile 2
lamzuctl profile set 2

# Set DPI stage 3 on current profile
lamzuctl dpi stage 3

# Show battery level
lamzuctl battery
```

### Stream Deck

1. Drag "Mouse Control" action to a button
2. Select mode: Profile Cycle, DPI Cycle, or Battery Monitor
3. For Profile Cycle: Check the profiles you want to cycle through
4. Optional: Enable "Show battery as background bar" for visual battery indicator

## Building from Source

### Requirements

- Rust 1.70+
- Windows 10+

### Build

```bash
# CLI only
cargo build --release

# Stream Deck plugin
cargo build --release --features streamdeck --bin lamzu-streamdeck

# Package Stream Deck plugin
.\build-plugin.ps1 -Release
```

The packaged plugin will be created as `io.github.denzonl.lamzuctl.streamDeckPlugin`.

## Technical Notes

- The HID protocol was reverse-engineered for interoperability purposes
- The plugin polls device state every 15 seconds when actions are visible
- Device communication uses HID feature reports on the control interface

## Reporting Issues

Please report bugs and issues via [GitHub Issues](https://github.com/DenzoNL/lamzuctl/issues).

For security vulnerabilities, please open a GitHub issue or contact the maintainer directly.

## License

[MIT](LICENSE)

---

*This is a hobby project with no guaranteed support or updates.*
