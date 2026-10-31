// Naygo — parser portable de /proc/self/mountinfo para el adaptador Linux.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Parser puro de mountinfo. No abre archivos ni hace syscalls: el adaptador Linux
//! debe leer `/proc/self/mountinfo` desde un worker y pasar el reader acá para no
//! bloquear el event loop. Las rutas se conservan como bytes para no perder nombres
//! POSIX que no sean UTF-8. Las líneas malformadas se cuentan y se omiten; una falla
//! real de lectura se propaga.

use std::io::{self, BufRead};

/// Una entrada de `/proc/self/mountinfo`, con campos de ruta/source preservados en bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountInfoEntry {
    pub mount_id: u64,
    pub parent_id: u64,
    pub major: u32,
    pub minor: u32,
    /// Raíz dentro del filesystem; relevante para bind mounts.
    pub root: Vec<u8>,
    /// Punto de montaje absoluto en el namespace del proceso.
    pub mount_point: Vec<u8>,
    pub mount_options: Vec<u8>,
    pub optional_fields: Vec<Vec<u8>>,
    pub filesystem_type: Vec<u8>,
    pub source: Vec<u8>,
    pub super_options: Vec<u8>,
}

/// Resultado tolerante del parseo: un registro inválido no oculta los montajes válidos.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MountInfoSnapshot {
    pub entries: Vec<MountInfoEntry>,
    pub malformed_lines: usize,
}

/// Parsea mountinfo en streaming y tolera líneas aisladas inválidas.
///
/// El llamador debe ejecutar la lectura del archivo en un worker. Esta función sólo
/// consume el reader recibido y no toca el filesystem por sí misma.
pub fn parse_mountinfo<R: BufRead>(reader: R) -> io::Result<MountInfoSnapshot> {
    let mut snapshot = MountInfoSnapshot::default();
    for line in reader.split(b'\n') {
        let mut line = line?;
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match parse_line(&line) {
            Some(entry) => snapshot.entries.push(entry),
            None => snapshot.malformed_lines += 1,
        }
    }
    Ok(snapshot)
}

fn parse_line(line: &[u8]) -> Option<MountInfoEntry> {
    let fields: Vec<&[u8]> = line
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty())
        .collect();
    // mountinfo: id parent major:minor root mountpoint options [optional...] -
    //            fstype source superoptions
    let separator = fields.iter().position(|field| *field == b"-")?;
    if separator < 6 || fields.len() < separator + 4 {
        return None;
    }

    let mount_id = parse_decimal(fields[0])?;
    let parent_id = parse_decimal(fields[1])?;
    let (major, minor) = parse_device_id(fields[2])?;
    let root = decode_mount_field(fields[3]);
    let mount_point = decode_mount_field(fields[4]);
    if !root.starts_with(b"/") || !mount_point.starts_with(b"/") {
        return None;
    }

    Some(MountInfoEntry {
        mount_id,
        parent_id,
        major,
        minor,
        root,
        mount_point,
        mount_options: fields[5].to_vec(),
        optional_fields: fields[6..separator]
            .iter()
            .map(|field| field.to_vec())
            .collect(),
        filesystem_type: decode_mount_field(fields[separator + 1]),
        source: decode_mount_field(fields[separator + 2]),
        super_options: fields[separator + 3].to_vec(),
    })
}

fn parse_decimal(field: &[u8]) -> Option<u64> {
    if field.is_empty() || !field.iter().all(u8::is_ascii_digit) {
        return None;
    }
    field.iter().try_fold(0u64, |value, digit| {
        value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))
    })
}

fn parse_device_id(field: &[u8]) -> Option<(u32, u32)> {
    let colon = field.iter().position(|byte| *byte == b':')?;
    let major = u32::try_from(parse_decimal(&field[..colon])?).ok()?;
    let minor = u32::try_from(parse_decimal(&field[colon + 1..])?).ok()?;
    Some((major, minor))
}

/// Decodifica sólo los escapes definidos por mountinfo. Los escapes desconocidos se
/// conservan byte por byte en vez de reinterpretarse o reemplazarse.
fn decode_mount_field(raw: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'\\' && index + 3 < raw.len() {
            let digits = &raw[index + 1..index + 4];
            if digits.iter().all(|digit| (b'0'..=b'7').contains(digit)) {
                let value = u16::from(digits[0] - b'0') * 64
                    + u16::from(digits[1] - b'0') * 8
                    + u16::from(digits[2] - b'0');
                if matches!(value, 0o11 | 0o12 | 0o40 | 0o134) {
                    decoded.push(value as u8);
                    index += 4;
                    continue;
                }
            }
        }
        decoded.push(raw[index]);
        index += 1;
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_mount_record_and_optional_fields() {
        let text = b"36 25 8:1 / /boot rw,relatime shared:7 master:2 - ext4 /dev/sda1 rw\n";
        let parsed = parse_mountinfo(Cursor::new(text)).unwrap();
        assert_eq!(parsed.malformed_lines, 0);
        assert_eq!(parsed.entries.len(), 1);
        let entry = &parsed.entries[0];
        assert_eq!((entry.mount_id, entry.parent_id), (36, 25));
        assert_eq!((entry.major, entry.minor), (8, 1));
        assert_eq!(entry.root, b"/");
        assert_eq!(entry.mount_point, b"/boot");
        assert_eq!(
            entry.optional_fields,
            vec![b"shared:7".to_vec(), b"master:2".to_vec()]
        );
        assert_eq!(entry.filesystem_type, b"ext4");
        assert_eq!(entry.source, b"/dev/sda1");
    }

    #[test]
    fn decodes_mountinfo_escapes_in_root_mountpoint_and_source() {
        let text = b"41 25 0:40 /sub\\040root /mnt/drive\\040one\\134two rw - fuse /run/media/disk\\011one rw\n";
        let parsed = parse_mountinfo(Cursor::new(text)).unwrap();
        let entry = &parsed.entries[0];
        assert_eq!(entry.root, b"/sub root");
        assert_eq!(entry.mount_point, b"/mnt/drive one\\two");
        assert_eq!(entry.source, b"/run/media/disk\tone");
    }

    #[test]
    fn preserves_non_utf8_bytes_and_unknown_escapes() {
        let mut text = b"42 25 0:40 / /mnt/".to_vec();
        text.push(0xff);
        text.extend_from_slice(b"\\777 rw - fuse ");
        text.push(0xfe);
        text.extend_from_slice(b"\\777 rw\n");
        let parsed = parse_mountinfo(Cursor::new(text)).unwrap();
        let entry = &parsed.entries[0];
        assert_eq!(
            entry.mount_point,
            [b"/mnt/".as_slice(), &[0xff], b"\\777"].concat()
        );
        assert_eq!(entry.source, [b"".as_slice(), &[0xfe], b"\\777"].concat());
    }

    #[test]
    fn keeps_bind_mount_root_and_nested_mountpoint_without_normalizing() {
        let text = b"57 25 8:1 /projects/naygo /mnt/data/nested rw shared:1 - ext4 /dev/sda1 rw\n";
        let parsed = parse_mountinfo(Cursor::new(text)).unwrap();
        let entry = &parsed.entries[0];
        assert_eq!(entry.root, b"/projects/naygo");
        assert_eq!(entry.mount_point, b"/mnt/data/nested");
    }

    #[test]
    fn omits_malformed_lines_but_keeps_other_mounts() {
        let text = b"\nnot-a-mount-record\n36 25 8:1 / /boot rw - ext4 /dev/sda1 rw\n";
        let parsed = parse_mountinfo(Cursor::new(text)).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.malformed_lines, 1);
    }

    #[test]
    fn rejects_overflow_and_non_absolute_paths() {
        let text = b"18446744073709551616 25 8:1 / /boot rw - ext4 /dev/sda1 rw\n36 25 8:1 relative /boot rw - ext4 /dev/sda1 rw\n";
        let parsed = parse_mountinfo(Cursor::new(text)).unwrap();
        assert!(parsed.entries.is_empty());
        assert_eq!(parsed.malformed_lines, 2);
    }
}
