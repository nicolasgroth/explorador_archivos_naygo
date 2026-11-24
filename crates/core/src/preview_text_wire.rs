// Naygo — texto resaltado acotado; el parser de sintaxis permanece en el lector aislado.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{invalid, read_bytes, read_u32, TEXT_MAX};
use crate::highlight::{HlLine, HlSpan};
use std::io::{self, Read, Write};

// Length/count bounds apply before every allocation. Text and spans together <=2 MiB;
// at most 65,536 span structs, not an arbitrary deserialized object tree.
const SPANS_MAX: usize = 65_536;
fn text(reader: &mut impl Read, remaining: &mut usize) -> io::Result<String> {
    let length = read_u32(reader)? as usize;
    *remaining = remaining.checked_sub(length).ok_or_else(invalid)?;
    String::from_utf8(read_bytes(reader, length)?).map_err(|_| invalid())
}
fn put(writer: &mut impl Write, value: &str) -> io::Result<()> {
    writer.write_all(&(value.len() as u32).to_le_bytes())?;
    writer.write_all(value.as_bytes())
}
fn validate(text: &str, lines: &[HlLine]) -> io::Result<()> {
    if text.len() > TEXT_MAX
        || lines.len() > crate::preview::TEXT_MAX_LINES
        || lines.iter().map(|l| l.spans.len()).sum::<usize>() > SPANS_MAX
    {
        return Err(invalid());
    }
    let mut originals = text.split_terminator('\n');
    for line in lines {
        let mut original = originals.next().ok_or_else(invalid)?;
        for span in &line.spans {
            if span.text.is_empty() {
                return Err(invalid());
            }
            original = original.strip_prefix(&span.text).ok_or_else(invalid)?;
        }
        if !original.is_empty() {
            return Err(invalid());
        }
    }
    if originals.next().is_some() {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn write(writer: &mut impl Write, text: &str, lines: &[HlLine]) -> io::Result<()> {
    validate(text, lines)?;
    put(writer, text)?;
    writer.write_all(&(lines.len() as u32).to_le_bytes())?;
    for line in lines {
        writer.write_all(&(line.spans.len() as u32).to_le_bytes())?;
        for span in &line.spans {
            writer.write_all(&[span.color.0, span.color.1, span.color.2])?;
            put(writer, &span.text)?;
        }
    }
    Ok(())
}
pub(super) fn read(reader: &mut impl Read) -> io::Result<(String, Vec<HlLine>)> {
    let mut original_remaining = TEXT_MAX;
    let original = text(reader, &mut original_remaining)?;
    let count = read_u32(reader)? as usize;
    if count > crate::preview::TEXT_MAX_LINES {
        return Err(invalid());
    }
    let mut remaining = TEXT_MAX;
    let mut spans_remaining = SPANS_MAX;
    let mut lines = Vec::new();
    lines.try_reserve_exact(count).map_err(|_| invalid())?;
    for _ in 0..count {
        let count = read_u32(reader)? as usize;
        spans_remaining = spans_remaining.checked_sub(count).ok_or_else(invalid)?;
        let mut spans = Vec::new();
        spans.try_reserve_exact(count).map_err(|_| invalid())?;
        for _ in 0..count {
            let mut color = [0; 3];
            reader.read_exact(&mut color)?;
            spans.push(HlSpan {
                text: text(reader, &mut remaining)?,
                color: (color[0], color[1], color[2]),
            });
        }
        lines.push(HlLine { spans });
    }
    validate(&original, &lines)?;
    Ok((original, lines))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlighted_unicode_roundtrip_and_truncated_body() {
        let source = "<a>á中</a>\n\n";
        let lines = crate::highlight::highlight(source, crate::preview::CodeLang::Xml);
        let mut bytes = Vec::new();
        write(&mut bytes, source, &lines).unwrap();
        assert_eq!(read(&mut bytes.as_slice()).unwrap(), (source.into(), lines));
        for end in 0..bytes.len() {
            assert!(read(&mut &bytes[..end]).is_err());
        }
    }
    #[test]
    fn rejects_oversized_counts_and_diverging_copy_text() {
        let mut bytes = Vec::new();
        put(&mut bytes, "x").unwrap();
        bytes.extend(251u32.to_le_bytes());
        assert!(read(&mut bytes.as_slice()).is_err());
        bytes.truncate(5);
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(u32::MAX.to_le_bytes());
        assert!(read(&mut bytes.as_slice()).is_err());
        let lines = vec![HlLine {
            spans: vec![HlSpan {
                text: "hidden".into(),
                color: (1, 2, 3),
            }],
        }];
        assert!(write(&mut Vec::new(), "shown", &lines).is_err());
    }
}
