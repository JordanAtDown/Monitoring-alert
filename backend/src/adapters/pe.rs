//! Windows executables carry a "subsystem" in their header: *console* (a console
//! window is created when it starts) or *GUI* (no window at all). The background
//! process must not flash a console at every logon, so the installer makes a copy
//! of the executable whose subsystem is switched to GUI — the same edit as
//! `editbin /SUBSYSTEM:WINDOWS`, with no extra tool, dependency or `unsafe`.

use anyhow::{bail, Result};

const SUBSYSTEM_WINDOWS_GUI: u16 = 2;
/// Offset of `Subsystem` in the optional header (identical for PE32 and PE32+).
const SUBSYSTEM_OFFSET: usize = 68;
const OPTIONAL_HEADER_OFFSET: usize = 4 + 20; // "PE\0\0" + COFF file header

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

/// Returns a copy of the executable `exe` that runs without a console window.
pub fn windows_gui_copy(exe: &[u8]) -> Result<Vec<u8>> {
    if exe.get(..2) != Some(b"MZ") {
        bail!("not a Windows executable (no MZ header)");
    }
    let pe = exe
        .get(0x3C..0x40)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .filter(|&pe| exe.get(pe..pe + 4) == Some(b"PE\0\0"));
    let Some(pe) = pe else {
        bail!("not a Windows executable (no PE header)");
    };
    let optional = pe + OPTIONAL_HEADER_OFFSET;
    // 0x10b = PE32, 0x20b = PE32+
    if !matches!(read_u16(exe, optional), Some(0x10b | 0x20b)) {
        bail!("unsupported PE optional header");
    }
    let at = optional + SUBSYSTEM_OFFSET;
    if read_u16(exe, at).is_none() {
        bail!("truncated PE header");
    }
    let mut copy = exe.to_vec();
    copy[at..at + 2].copy_from_slice(&SUBSYSTEM_WINDOWS_GUI.to_le_bytes());
    Ok(copy)
}

/// The subsystem of a PE file (2 = GUI, 3 = console).
#[cfg(test)]
pub fn subsystem(exe: &[u8]) -> Option<u16> {
    let pe = u32::from_le_bytes(exe.get(0x3C..0x40)?.try_into().ok()?) as usize;
    read_u16(exe, pe + OPTIONAL_HEADER_OFFSET + SUBSYSTEM_OFFSET)
}
