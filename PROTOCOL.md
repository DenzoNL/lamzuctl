# Lamzu Mouse HID Protocol Documentation

## ⚠️ DISCLAIMERS

### AI-Generated Documentation
This documentation was generated through AI-assisted reverse engineering of the official Lamzu software JavaScript implementation. While care has been taken to ensure accuracy, there may be errors, omissions, or misinterpretations in the protocol specification.

### Use At Your Own Risk
**WARNING: Improper use of these HID commands may permanently damage or brick your device.**

- This is **unofficial** and **reverse-engineered** documentation
- Commands have **not been tested** against actual hardware
- Certain operations (firmware updates, flash writes, factory resets) are **potentially destructive**
- The authors take **NO responsibility** for any damage to your hardware
- **Always backup your device configuration** before experimenting
- Test on non-critical devices first
- Use this documentation at your own risk

### Recommended Safety Practices

1. **Read-only operations first**: Start by implementing and testing only read/get commands (opcodes >= 0x80)
2. **Verify before writing**: Always read current values before attempting to write new ones
3. **Avoid dangerous commands**: Be extremely cautious with firmware updates, flash operations, and factory resets
4. **Test incrementally**: Implement one command at a time and verify behavior
5. **Know your device**: Ensure you're targeting the correct device ID and protocol variant

**By using this documentation, you acknowledge these risks and agree that you do so entirely at your own discretion and risk.**

---

## Overview

This document describes the HID protocol used to communicate with Lamzu gaming mice. The protocol uses HID Feature Reports with a report size of 64 bytes (reportCount=64) and Report ID 0. Additionally, Input Reports with Report ID 4 are used for receiving data.

## General Packet Structure

All commands use 64-byte packets with the following general structure:

```
Byte 0: [Usually 0x00 - Report ID placeholder]
Byte 1: [Device response marker: 0xA0-0xAF (160-175) for valid responses]
Byte 2: Device ID (typically 0x02 for mouse, 0x00 for dongle)
Byte 3: Payload length
Byte 4: Command category
Byte 5: Command opcode (bit 7: 0=write/set, 1=read/get)
Byte 6+: Command-specific parameters
```

> **Implementation Note (hidapi):** When using libraries like `hidapi` that include the Report ID in read/write buffers, all byte offsets shift by 1. For example, the response marker appears at `response[1]` instead of `response[0]`, and data offsets documented as `[7]` would be at `response[8]`. The documentation below uses the wire format (without Report ID prefix).

### Protocol Variants

The protocol has two variants detected via `hidIndex`:
- **hidIndex=0**: Response data starts at byte offset 6
- **hidIndex=1**: Response data starts at byte offset 5

Detection: If `response[0] >= 160`, use hidIndex=1, otherwise hidIndex=0.

## HID Report Configuration

- **Feature Reports**: Report ID 0, 64 bytes
  - Used for bidirectional command/response communication
  - Accessed via `sendFeatureReport()` and `receiveFeatureReport()`
- **Input Reports**: Report ID 4
  - Used for receiving asynchronous data from device
  - Accessed via `oninputreport` event handler

## Command Categories

Commands are organized by category (byte 4):

| Category | Description |
|----------|-------------|
| 0x00 | General device commands |
| 0x01 | Configuration read/write |
| 0x02 | Lighting effects |
| 0x03 | Button configuration |
| 0x04 | Macro management |

## Command Opcodes

Commands use opcodes in byte 5, where bit 7 indicates direction:
- **Bit 7 = 0**: Write/Set command (0x00-0x7F)
- **Bit 7 = 1**: Read/Get command (0x80-0xFF, 128-255)

### Common Command Pairs

| Write (Set) | Read (Get) | Function |
|-------------|------------|----------|
| 0x00 | 0x80 | Polling rate |
| 0x01 | 0x81 | DPI stage info |
| 0x02 | 0x82 | Active DPI / DPI indicator |
| 0x04 | 0x84 | DPI indicator / Angle snap |
| 0x05 | 0x85 | Profile ID |
| 0x07 | 0x87 | Sleep time |
| 0x08 | 0x88 | Debounce time / LOD |
| 0x09 | 0x89 | Motion sync |
| 0x0D | 0x8D | Reset profile |
| 0x81 | - | Firmware version (0x81 = 129) |
| 0x82 | - | EID/Encryption ID (0x82 = 130) |
| 0x83 | - | Battery level (0x83 = 131) |
| 0x85 | - | Get profile ID (0x85 = 133) |

## Detailed Command Reference

### Device Information Commands

#### Get Firmware Version (0x81)

**Request:**
```
[0] = 0x00 (Report ID)
[2] = 0x02 (Device ID: 0x00=dongle, 0x02=mouse)
[3] = 0x10 (16 - payload length)
[5] = 0x81 (129 - get version opcode)
```

**Response (hidIndex=0):**
```
[1] = 0xA1 (161 - success marker)
[6] = 0x81 (opcode echo)
[7] = major version
[8] = minor version
[9] = patch version
[10] = build version
```

**Response (hidIndex=1):**
```
[0] = 0xA1 (161 - success marker)
[5] = 0x81 (opcode echo)
[6] = major version
[7] = minor version
[8] = patch version
[9] = build version
```

#### Get EID/Encryption ID (0x82)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x01 (payload length)
[5] = 0x82 (130 - get EID opcode)
```

**Response (hidIndex=0):**
```
[6] = 0x82 (opcode echo)
[7] = EID value
```

**Response (hidIndex=1):**
```
[5] = 0x82 (opcode echo)
[6] = EID value
```

#### Get Battery Level (0x83)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x00 (category - general)
[5] = 0x83 (131 - get battery opcode)
```

**Response (hidIndex=0):**
```
[1] = 0xA0-0xAF (success marker)
[4] = 0x02
[6] = 0x83 (opcode echo)
[7] = charging status (0=not charging, 1=charging)
[8] = battery percentage (0-100)
```

**Response (hidIndex=1):**
```
[0] = 0xA0-0xAF (success marker)
[3] = 0x02
[5] = 0x83 (opcode echo)
[6] = charging status
[7] = battery percentage
```

> **Verified:** The order is charging status first, then percentage (opposite of original documentation).

### Profile Management

#### Set Profile ID (0x05)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x01 (payload length)
[5] = 0x05 (set profile opcode)
[6] = profile_id (0-based, typically 0-4)
```

**Response:**
```
[7-hidIndex] = new profile ID
```

#### Get Profile ID (0x85)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x01 (payload length)
[5] = 0x85 (133 - get profile opcode)
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
[6-hidIndex] = 0x85 (opcode echo)
[7-hidIndex] = current profile ID
```

### Polling Rate Configuration

#### Set Polling Rate (0x00)

**Request (New Protocol):**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x01 (category)
[5] = 0x00 (set polling rate opcode)
[6] = profile_id
[7] = polling_rate_value
```

**Request (Old Protocol):**
```
[2] = 0x02
[3] = 0x02
[4] = 0x01
[5] = 0x00
[6] = polling_rate_value
```

**Polling Rate Values:**
- 0x01 = 1000 Hz (1ms)
- 0x02 = 500 Hz (2ms)
- 0x04 = 250 Hz (4ms)
- 0x08 = 125 Hz (8ms)
- 0x10 = Special value (converted to 0x01 / 1000 Hz)
- 0x40 = 4000 Hz (4K devices)
- 0x80 = 8000 Hz (8K devices)

#### Get Polling Rate (0x80)

**Request (New Protocol):**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x01 (category)
[5] = 0x80 (128 - get polling rate opcode)
[6] = profile_id
```

**Request (Old Protocol):**
```
[2] = 0x02
[3] = 0x02
[4] = 0x01
[5] = 0x80
[6] = 0x00
```

**Response (New Protocol):**
```
[8-hidIndex] = polling rate value (convert 0x10 to 0x01)
```

**Response (Old Protocol):**
```
[7-hidIndex] = polling rate value (convert 0x10 to 0x01)
```

### DPI Configuration

#### Get DPI Stage Info (0x81)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x0A (10 - payload length)
[4] = 0x01 (category)
[5] = 0x81 (129 - get DPI info opcode)
[6] = profile_id
[7] = stage_count (typically 6)
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
[6-hidIndex] = 0x81 (opcode echo)
[8-hidIndex] = number_of_stages
[9-hidIndex, 10-hidIndex] = DPI X stage 1 (big endian u16)
[11-hidIndex, 12-hidIndex] = DPI Y stage 1 (big endian u16)
[13-hidIndex, 14-hidIndex] = DPI X stage 2
[15-hidIndex, 16-hidIndex] = DPI Y stage 2
... (4 bytes per stage)
```

#### Set DPI Stage Info (0x01)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x1A (26 - payload length)
[4] = 0x01 (category)
[5] = 0x01 (set DPI info opcode)
[6] = profile_id
[7] = number_of_stages
[8+] = stage_data (array of bytes, 4 bytes per stage: X_hi, X_lo, Y_hi, Y_lo)
```

#### Get Active DPI Stage (0x82)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x01 (category)
[5] = 0x82 (130 - get active DPI opcode)
[6] = profile_id
```

**Response:**
```
[8-hidIndex] = active_dpi_stage (0-based index)
```

#### Set Active DPI Stage (0x02)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x01 (category)
[5] = 0x02 (set active DPI opcode)
[6] = profile_id
[7] = stage_index (0-based)
```

#### Set Active DPI Value (Combined Command)

This is a complex command that reads current DPI stages, modifies one, and writes back.

**Step 1: Get current DPI info (0x81)**
**Step 2: Modify the desired stage**
**Step 3: Write back with Set DPI Stage Info (0x01)**

#### Get DPI Maximum (0x8C)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x01 (category)
[5] = 0x8C (140 - get DPI max opcode)
```

**Response:**
```
[7-hidIndex, 8-hidIndex] = max_dpi (big endian u16)
```

### DPI Color Configuration

#### Get DPI Stage Colors (0x81)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x13 (19 - payload length)
[4] = 0x02 (category - lighting)
[5] = 0x81 (129 - get colors opcode)
[6] = profile_id
```

**Response:**
Returns array of RGB colors for each DPI stage

#### Set DPI Stage Colors (0x01)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x13 (19 - payload length)
[4] = 0x02 (category - lighting)
[5] = 0x01 (set colors opcode)
[6] = profile_id
[7+] = color_array (RGB bytes for each stage)
```

### Debounce Time

#### Get Debounce Time (0x88)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[5] = 0x88 (136 - get debounce opcode)
[6] = profile_id
```

**Response:**
```
[8-hidIndex] = debounce_time_ms
```

#### Set Debounce Time (0x08)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[5] = 0x08 (set debounce opcode)
[6] = profile_id
[7] = debounce_time_ms
```

### Sleep Time

#### Get Sleep Time (0x87)

**Request (New Protocol):**
```
[2] = 0x02 (Device ID)
[3] = 0x03 (payload length)
[5] = 0x87 (135 - get sleep time opcode)
[6] = profile_id
```

**Request (Old Protocol):**
```
[2] = 0x02
[3] = 0x03
[5] = 0x87
[6] = 0x00
```

**Response (New Protocol):**
```
value = (response[8-hidIndex] << 8) + response[9-hidIndex]
```

**Response (Old Protocol):**
```
value = (response[7-hidIndex] << 8) + response[8-hidIndex]
```

Returns sleep time in seconds.

#### Set Sleep Time (0x07)

**Request (New Protocol):**
```
[2] = 0x02 (Device ID)
[3] = 0x03 (payload length)
[5] = 0x07 (set sleep time opcode)
[6] = profile_id
[7] = time_seconds >> 8 (high byte)
[8] = time_seconds & 0xFF (low byte)
```

**Request (Old Protocol):**
```
[2] = 0x02
[3] = 0x03
[5] = 0x07
[6] = time_seconds >> 8
[7] = time_seconds & 0xFF
```

### Lift-Off Distance (LOD)

#### Get LOD (0x88)

**Request (New Protocol):**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x01 (category)
[5] = 0x88 (136 - get LOD opcode)
[6] = profile_id
```

**Request (Old Protocol):**
```
[2] = 0x02
[3] = 0x02
[4] = 0x01
[5] = 0x88
[6] = 0x00
```

**Response (New Protocol):**
```
[8-hidIndex] = LOD value
```

**Response (Old Protocol):**
```
[7-hidIndex] = LOD value
```

**LOD Value Encoding:**
- If value >= 1: Direct mm value
- If value < 1: (value * 10) | 0x80 (bit 7 set)

#### Set LOD (0x08)

**Request (New Protocol):**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x01 (category)
[5] = 0x08 (set LOD opcode)
[6] = profile_id
[7] = LOD value (encoded as above)
```

**Request (Old Protocol):**
```
[2] = 0x02
[3] = 0x02
[4] = 0x01
[5] = 0x08
[6] = LOD value
```

### Button Configuration

#### Set Button Function (0x00)

**Request:**
```
[2] = device_id (0x02 for mouse)
[3] = 0x00 (calculated: 5 + data_length)
[4] = 0x03 (category - button config)
[5] = 0x00 (set button opcode)
[6] = profile_id
[7] = button_id
[8] = 0x00 (reserved)
[9] = function_type
[10] = data_length
[11+] = function_data (variable length)
```

**Function Types (from Ka enum):**
- 0x00: Disable
- 0x01: Mouse Key
- 0x02: DPI Switch
- 0x03: Left/Right Scroll
- 0x04: Fire Key (rapid fire)
- 0x05: Shortcut Key
- 0x06: Macro
- 0x07: Report Rate Switch
- 0x08: Light Switch
- 0x09: Profile Switch
- 0x0A: DPI Lock
- 0x0B: Up/Down Scroll
- 0x100: Left Key (256)

#### Get Button Function (0x80)

**Request:**
```
[2] = device_id
[3] = 0x00 (calculated: 5 + data_length)
[4] = 0x03 (category - button config)
[5] = 0x80 (128 - get button opcode)
[6] = profile_id
[7] = button_id
[8] = 0x00 (reserved)
[9] = function_type
[10] = data_length
[11+] = function_data
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
Returns button configuration data
```

### Lighting Effects

#### Set Light Effect (0x00)

**Request:**
```
[2] = device_id
[3] = 5 + color_data_length (calculated)
[4] = 0x02 (category - lighting)
[5] = 0x00 (set light effect opcode)
[6] = profile_id
[7] = effect_mode
[8] = brightness
[9] = speed
[10] = state (on/off)
[11+] = color_data (variable length)
```

#### Get Light Effect (0x80)

**Request:**
```
[2] = device_id
[3] = 5 + color_data_length
[4] = 0x02 (category - lighting)
[5] = 0x80 (128 - get light effect opcode)
[6] = profile_id
```

**Response:**
```
[8-hidIndex] = effect_mode
[9-hidIndex] = brightness
[10-hidIndex] = speed
[11-hidIndex] = state
[12-hidIndex+] = color_data
```

### Macro Management

#### Allocate Macro Data Size (0x01)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x06 (payload length)
[4] = 0x04 (category - macro)
[5] = 0x01 (allocate opcode)
[6] = macro_id >> 8 (high byte of u16)
[7] = macro_id & 0xFF (low byte of u16)
[8] = size >> 24 (byte 0 of u32 size)
[9] = size >> 16 (byte 1)
[10] = size >> 8 (byte 2)
[11] = size & 0xFF (byte 3)
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
```

#### Set Macro Data (0x03)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 2 + 4 + 1 + data_length (calculated)
[4] = 0x04 (category - macro)
[5] = 0x03 (set macro data opcode)
[6] = macro_id >> 8
[7] = macro_id & 0xFF
[8] = offset >> 24 (u32 offset)
[9] = offset >> 16
[10] = offset >> 8
[11] = offset & 0xFF
[12] = chunk_length
[13+] = macro_data (chunk_length bytes)
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
```

#### Get Macro Data Size (0x81)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x06 (payload length)
[4] = 0x04 (category - macro)
[5] = 0x81 (129 - get macro size opcode)
[6] = macro_id >> 8
[7] = macro_id & 0xFF
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
Returns macro size information
```

#### Get Macro Data (0x83)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = size_data[6] + 7 (from previous size query)
[4] = 0x04 (category - macro)
[5] = 0x83 (131 - get macro data opcode)
[6] = macro_id >> 8
[7] = macro_id & 0xFF
[8-11] = offset and length from size_data
[12] = chunk size from size_data
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
Returns macro data chunk
```

#### Delete Macro (0x02)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x04 (category - macro)
[5] = 0x02 (delete macro opcode)
[6] = macro_id >> 8
[7] = macro_id & 0xFF
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
```

### Advanced Settings

#### Get Button Combine (0x81)

**Request (New Protocol):**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x03 (category)
[5] = 0x81 (129 - get button combine opcode)
[6] = profile_id
```

**Request (Old Protocol):**
```
[2] = 0x02
[3] = 0x02
[4] = 0x03
[5] = 0x81
[6] = 0x00
```

**Response (New Protocol):**
```
returns: response[8-hidIndex] == 1
```

**Response (Old Protocol):**
```
returns: response[7-hidIndex] == 1
```

#### Set Button Combine (0x01)

**Request (New Protocol):**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x03 (category)
[5] = 0x01 (set button combine opcode)
[6] = profile_id
[7] = enable (0 or 1)
```

**Request (Old Protocol):**
```
[2] = 0x02
[3] = 0x02
[4] = 0x03
[5] = 0x01
[6] = enable
```

#### Get/Set DPI Indicator (0x84 / 0x04)

Follow same pattern as Button Combine with category 0x02.

#### Get/Set DPI XY On/Off (0x8D / 0x0D)

Follow same pattern as Button Combine with category 0x01.

#### Get/Set Angle Tune (0x94 / 0x14)

**Get Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x01 (category)
[5] = 0x94 (148 - get angle tune opcode)
[6] = profile_id (if new protocol) or 0x00
```

**Set Request:**
```
[2] = 0x02
[3] = 0x02
[4] = 0x01
[5] = 0x14 (20 - set angle tune opcode)
[6] = profile_id (if new protocol)
[7] = angle_value
```

**Angle Value Encoding:**
- If angle > 0: Use value directly
- If angle <= 0: Use 255 - abs(angle) + 1 (two's complement)

#### Get/Set Angle Snap (0x84 / 0x04)

Same pattern as Button Combine, returns boolean (1 = enabled).

#### Get/Set Motion Sync (0x89 / 0x09)

Same pattern as Button Combine, returns boolean (1 = enabled).

#### Get/Set Tracking Mode (0x93 / 0x13)

Same pattern as Button Combine, returns boolean (1 = enabled).

#### Get/Set Hyper Mode (0x8B / 0x0B)

Same pattern as Button Combine, returns boolean (1 = enabled).

#### Get/Set Ripple Control (0x8A / 0x0A)

Same pattern as Button Combine, returns boolean (1 = enabled).

### Device Management

#### Reset Profile (0x0D)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x01 (payload length)
[4] = 0x00 (category)
[5] = 0x0D (13 - reset profile opcode)
[6] = profile_id
```

#### Default Device (Factory Reset)

**Request:**
```
[2] = 0x02 (Device ID)
[3] = 0x02 (payload length)
[4] = 0x00 (category)
[5] = 0x00 (opcode)
[6] = 0xC0 (192)
[7] = 0x01
```

#### Get Device PID

**Request:**
```
[2] = 0x01 (Device ID)
[3] = 0x06 (payload length)
[5] = 0x8B (139 - get PID opcode)
[6] = 0x02
```

**Response:**
```
[1-hidIndex] = 0xA1 (success)
[9-hidIndex, 10-hidIndex] = VID (big endian u16)
[11-hidIndex, 12-hidIndex] = PID (big endian u16)
```

## Configuration Offsets (Flash Memory)

The following offsets are used when reading/writing configuration from flash memory:

| Offset | Name | Description |
|--------|------|-------------|
| 0 | ReportRate | Polling rate configuration |
| 2 | maxDpiStage | Maximum DPI stages |
| 4 | CurrentDPI | Current DPI stage index |
| 10 | LOD | Lift-off distance |
| 12 | DPIValue | DPI values start |
| 44 | DPIColor | DPI color values start |
| 76 | DPIEffectMode | DPI effect mode |
| 78 | DPIEffectBrightness | DPI effect brightness |
| 80 | DPIEffectSpeed | DPI effect speed |
| 82 | DPIEffectState | DPI effect state |
| 96 | KeyFunction | Button functions start |
| 160 | Light | Lighting configuration |
| 169 | DebounceTime | Debounce time |
| 171 | MotionSync | Motion sync setting |
| 173 | SleepTime | Sleep time |
| 175 | Angle | Angle tune |
| 177 | Ripple | Ripple control |
| 179 | MovingOffLight | Moving off light |
| 181 | PerformanceState | Performance state |
| 183 | Performance | Performance setting |
| 185 | SensorMode | Sensor mode |
| 256 | ShortcutKey | Shortcut key data start |
| 768 | Macro | Macro data start |

## Extended Commands (eS enum)

Additional commands for advanced operations:

| Value | Name | Description |
|-------|------|-------------|
| 1 | EncryptionData | Encryption data operations |
| 2 | PCDriverStatus | PC driver status |
| 3 | DeviceOnLine | Device online status |
| 4 | BatteryLevel | Battery level query |
| 5 | DongleEnterPair | Enter pairing mode |
| 6 | GetPairState | Get pairing state |
| 7 | WriteFlashData | Write to flash memory |
| 8 | ReadFlashData | Read from flash memory |
| 9 | ClearSetting | Clear settings |
| 10 | StatusChanged | Status change notification |
| 11 | SetDeviceVidPid | Set VID/PID |
| 12 | SetDeviceDescriptorString | Set descriptor string |
| 13 | EnterUsbUpdateMode | Enter firmware update mode |
| 14 | GetCurrentConfig | Get current configuration |
| 15 | SetCurrentConfig | Set current configuration |
| 16 | ReadCIDMID | Read CID/MID |
| 17 | EnterMTKMode | Enter MTK mode |
| 18 | ReadVersionID | Read version ID |
| 20 | Set4KDongleRGB | Set 4K dongle RGB |
| 21 | Get4KDongleRGBValue | Get 4K dongle RGB value |
| 22 | SetLongRangeMode | Set long range mode |
| 23 | GetLongRangeMode | Get long range mode |
| 176 | MusicColorful | Music colorful mode |
| 177 | MusicSingleColor | Music single color mode |
| 240 | WriteKBCIdMID | Write KBC ID/MID |
| 241 | ReadKBCIdMID | Read KBC ID/MID |

## Timing and Delays

- **commonDelay**: Default delay between commands (typically 100ms)
- **programDelay**: Delay for programming operations
- **programDelay4K**: Delay for 4K-specific operations
- **verifyDelay**: Delay for verification operations

## Response Validation

Valid responses are indicated by:
1. Response byte `[1-hidIndex]` is in range 0xA0-0xAF (160-175)
2. Opcode echo in response matches request opcode
3. Some commands implement retry logic with exponential backoff

> **Verified:** The Maya X 8K returns markers like 0xA0, 0xA1, 0xA2, 0xA3 depending on the command. Accept the full 0xA0-0xAF range.

## Firmware Update Protocol

Firmware updates use a special encrypted packet format:

1. Parse Intel HEX file
2. Divide data into 16-byte chunks
3. Encrypt chunks with XOR 0x55 (85) starting at byte 12
4. Build 65-byte packets:
   - Byte 0: Report ID
   - Bytes 1-2: Header
   - Byte 3: Chunk size + 5
   - Byte 4: 0xB0 (176)
   - Byte 5: 0x02
   - Byte 6: Data length
   - Bytes 7-10: Address (4 bytes, little endian)
   - Bytes 11+: Encrypted data
5. Send via Feature Report

## Protocol Notes

1. **Device ID**: 0x00 for dongle, 0x02 for mouse
2. **Profile ID**: 1-based index (1-5 for 5 profiles) - device returns 1-indexed values
3. **DPI Stage**: 1-based index - device returns 1-indexed values
4. **Byte Order**: Most multi-byte values use big-endian unless noted
5. **Response Timeout**: Typically 100ms, with retry capability
6. **Report ID**: Always 0 for Feature Reports
7. **Buffer Size**: Always 64 bytes, unused bytes should be 0x00
8. **Payload Length**: Byte 3 contains the length of bytes 4 onwards
9. **Response Marker**: Accept 0xA0-0xAF (160-175) as valid response markers, not just 0xA1
10. **Feature Reports**: Use `send_feature_report()` / `get_feature_report()` for communication, with a small delay (~10ms) between send and receive

## Implementation Checklist

For implementing this protocol in Rust:

- [x] Define packet structures with 64-byte arrays
- [ ] Implement hidIndex detection logic
- [x] Create command builders for each operation
- [x] Implement response parsers with offset calculation
- [ ] Add retry logic with timeouts
- [ ] Support both old and new protocol variants
- [ ] Implement firmware update functionality
- [ ] Add macro recording/playback
- [x] Support profile management (read)
- [ ] Implement button remapping
- [x] Add DPI configuration (read)
- [ ] Support RGB lighting control
- [x] Get polling rate (read)
- [x] Get battery status (read)

### Verified Commands (Maya X 8K)

The following commands have been tested and verified working:

| Command | Opcode | Status |
|---------|--------|--------|
| Get Profile | 0x85 | Verified |
| Get Polling Rate | 0x80 | Verified |
| Get Battery | 0x83 | Verified |
| Get Active DPI Stage | 0x82 | Verified |
| Get DPI Stages | 0x81 | Verified |
