//! Known Lamzu device database

/// Lamzu USB Vendor IDs
pub const LAMZU_VID: u16 = 0x373E;        // Maya X series
pub const LAMZU_AURORA_VID: u16 = 0x37B0; // Aurora series (TACHI, INCA, Maya, PARO, THORN, etc.)
pub const ATLANTIS_VID: u16 = 0x3554;     // Atlantis series

/// All known Lamzu vendor IDs
pub const LAMZU_VIDS: &[u16] = &[LAMZU_VID, LAMZU_AURORA_VID, ATLANTIS_VID];

/// Check if a vendor ID belongs to Lamzu
pub fn is_lamzu_vendor(vid: u16) -> bool {
    LAMZU_VIDS.contains(&vid)
}

/// Known Lamzu device information
#[derive(Debug, Clone, Copy)]
pub struct KnownDevice {
    /// Vendor ID
    pub vid: u16,
    /// Product ID
    pub pid: u16,
    /// Device name
    pub name: &'static str,
}

/// Database of known Lamzu devices
///
/// All known Lamzu devices use the same protocol and are supported.
/// Devices not in this list may still work - please report if they do!
///
/// Note: This list is based on Lamzu Aurora software config.
pub const KNOWN_DEVICES: &[KnownDevice] = &[
    // === Maya X series (VID 0x373E) ===
    KnownDevice { vid: 0x373E, pid: 0x001C, name: "Maya X (Wired)" },
    KnownDevice { vid: 0x373E, pid: 0x001D, name: "Maya X 1K Dongle" },
    KnownDevice { vid: 0x373E, pid: 0x001E, name: "Maya X 8K Dongle" },
    KnownDevice { vid: 0x373E, pid: 0x0016, name: "Maya X V2 8K Dongle" },

    // === Aurora series (VID 0x37B0) ===
    // TACHI
    KnownDevice { vid: 0x37B0, pid: 0x0005, name: "TACHI (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x000B, name: "TACHI 1K Dongle" },
    KnownDevice { vid: 0x37B0, pid: 0x000C, name: "TACHI 8K Dongle" },

    // INCA
    KnownDevice { vid: 0x37B0, pid: 0x0009, name: "INCA (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x000F, name: "INCA 1K Dongle" },
    KnownDevice { vid: 0x37B0, pid: 0x0010, name: "INCA 8K Dongle" },

    // Maya (Aurora)
    KnownDevice { vid: 0x37B0, pid: 0x0011, name: "Maya (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x0013, name: "Maya 1K Dongle" },
    KnownDevice { vid: 0x37B0, pid: 0x0015, name: "Maya 8K Dongle" },

    // PARO
    KnownDevice { vid: 0x37B0, pid: 0x0007, name: "PARO (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x000D, name: "PARO 1K Dongle" },
    KnownDevice { vid: 0x37B0, pid: 0x000E, name: "PARO 8K Dongle" },

    // THORN
    KnownDevice { vid: 0x37B0, pid: 0x0017, name: "THORN (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x0019, name: "THORN 1K Dongle" },
    KnownDevice { vid: 0x37B0, pid: 0x001B, name: "THORN 8K Dongle" },

    // THORN V2
    KnownDevice { vid: 0x37B0, pid: 0x0021, name: "THORN V2 (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x0023, name: "THORN V2 8K Dongle" },
    KnownDevice { vid: 0x37B0, pid: 0x0030, name: "THORN V2 54H20 (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x0032, name: "THORN V2 54H20 8K Dongle" },

    // Tachi Lite
    KnownDevice { vid: 0x37B0, pid: 0x001C, name: "Tachi Lite (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x001E, name: "Tachi Lite 8K Dongle" },

    // Maya X V2 (Aurora VID)
    KnownDevice { vid: 0x37B0, pid: 0x001F, name: "Maya X V2 (Wired)" },

    // Atlantis OG Champion
    KnownDevice { vid: 0x37B0, pid: 0x0025, name: "Atlantis OG Champion (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x0027, name: "Atlantis OG Champion 8K Dongle" },

    // Atlantis Mini
    KnownDevice { vid: 0x37B0, pid: 0x0028, name: "Atlantis Mini (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x002A, name: "Atlantis Mini 8K Dongle" },
    KnownDevice { vid: 0x37B0, pid: 0x002B, name: "Atlantis Mini 1K Dongle" },

    // Maya M 54H20
    KnownDevice { vid: 0x37B0, pid: 0x002C, name: "Maya M 54H20 (Wired)" },
    KnownDevice { vid: 0x37B0, pid: 0x002E, name: "Maya M 54H20 8K Dongle" },

    // === Atlantis series (VID 0x3554) ===
    KnownDevice { vid: 0x3554, pid: 0xF50F, name: "Atlantis (Wired)" },
    KnownDevice { vid: 0x3554, pid: 0xF50D, name: "Atlantis 1K Dongle" },
    KnownDevice { vid: 0x3554, pid: 0xF510, name: "Atlantis 8K Dongle" },
];

/// Look up a device by its vendor and product ID
pub fn lookup_device(vid: u16, pid: u16) -> Option<&'static KnownDevice> {
    KNOWN_DEVICES.iter().find(|d| d.vid == vid && d.pid == pid)
}
