use crate::adapters::pe::{subsystem, windows_gui_copy};

const PE_OFFSET: usize = 0x80;

/// A header-only PE file with the given optional-header magic and subsystem.
pub fn fake_pe(magic: u16, subsystem: u16) -> Vec<u8> {
    let mut b = vec![0xAAu8; 0x400];
    b[0..2].copy_from_slice(b"MZ");
    b[0x3C..0x40].copy_from_slice(&(PE_OFFSET as u32).to_le_bytes());
    b[PE_OFFSET..PE_OFFSET + 4].copy_from_slice(b"PE\0\0");
    let optional = PE_OFFSET + 24;
    b[optional..optional + 2].copy_from_slice(&magic.to_le_bytes());
    b[optional + 68..optional + 70].copy_from_slice(&subsystem.to_le_bytes());
    b
}

#[test]
fn a_console_executable_becomes_a_gui_one() {
    for magic in [0x10b, 0x20b] {
        let console = fake_pe(magic, 3);
        let gui = windows_gui_copy(&console).unwrap();
        assert_eq!(subsystem(&console), Some(3));
        assert_eq!(subsystem(&gui), Some(2));
    }
}

#[test]
fn only_the_subsystem_field_changes() {
    let console = fake_pe(0x20b, 3);
    let gui = windows_gui_copy(&console).unwrap();
    assert_eq!(console.len(), gui.len());
    let differing: Vec<usize> = (0..console.len())
        .filter(|&i| console[i] != gui[i])
        .collect();
    assert_eq!(differing, [PE_OFFSET + 24 + 68]);
}

#[test]
fn it_is_idempotent() {
    let gui = windows_gui_copy(&fake_pe(0x20b, 3)).unwrap();
    assert_eq!(windows_gui_copy(&gui).unwrap(), gui);
}

#[test]
fn anything_that_is_not_a_pe_file_is_refused() {
    assert!(windows_gui_copy(b"").is_err());
    assert!(windows_gui_copy(b"#!/bin/sh\necho hi\n").is_err());

    let mut no_pe_signature = fake_pe(0x20b, 3);
    no_pe_signature[PE_OFFSET] = b'X';
    assert!(windows_gui_copy(&no_pe_signature).is_err());

    assert!(
        windows_gui_copy(&fake_pe(0x107, 3)).is_err(),
        "ROM image magic"
    );
    assert!(
        windows_gui_copy(&fake_pe(0x20b, 3)[..PE_OFFSET + 40]).is_err(),
        "truncated"
    );
}
