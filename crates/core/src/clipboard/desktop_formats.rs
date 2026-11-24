// Naygo — formatos de escritorio para portapapeles de archivos Linux.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Serialización pura de los metadatos copy/cut que acompañan `text/uri-list`.
//!
//! Nautilus usa `x-special/gnome-copied-files` (`copy|cut`, LF, URIs); Dolphin usa
//! `application/x-kde-cutselection` (`0` copia, `1` corte). Esta capa no accede al portapapeles.

use crate::clipboard::uri_list::{decode_file_uri_list, encode_file_uri_list, UriListError};

/// Codifica el formato de archivos de Nautilus, sin LF final.
pub fn encode_gnome_copied_files(paths: &[&[u8]], cut: bool) -> Result<Vec<u8>, UriListError> {
    let uris = encode_file_uri_list(paths)?;
    let mut payload = Vec::with_capacity(uris.len() + 5);
    payload.extend_from_slice(if cut { b"cut\n" } else { b"copy\n" });
    payload.extend(uris.into_iter().filter(|byte| *byte != b'\r'));
    if payload.last() == Some(&b'\n') {
        payload.pop();
    }
    Ok(payload)
}

/// Decodifica el formato de Nautilus a rutas locales y el indicador de corte.
pub fn decode_gnome_copied_files(input: &[u8]) -> Result<(Vec<Vec<u8>>, bool), UriListError> {
    let Some(separator) = input.iter().position(|byte| *byte == b'\n') else {
        return Err(UriListError {
            line: 1,
            kind: super::uri_list::UriListErrorKind::MissingPath,
        });
    };
    let cut = match &input[..separator] {
        b"copy" => false,
        b"cut" => true,
        _ => {
            return Err(UriListError {
                line: 1,
                kind: super::uri_list::UriListErrorKind::UnsupportedAction,
            })
        }
    };
    let paths = decode_file_uri_list(&input[separator + 1..])?;
    if paths.is_empty() {
        return Err(UriListError {
            line: 2,
            kind: super::uri_list::UriListErrorKind::MissingPath,
        });
    }
    Ok((paths, cut))
}

/// Codifica el indicador de corte de Dolphin: `1` para mover, `0` para copiar.
pub fn encode_kde_cutselection(cut: bool) -> [u8; 1] {
    [if cut { b'1' } else { b'0' }]
}

/// Decodifica el indicador de corte de Dolphin; bytes distintos de `0`/`1` se rechazan.
pub fn decode_kde_cutselection(input: &[u8]) -> Option<bool> {
    match input {
        [b'0'] => Some(false),
        [b'1'] => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::uri_list::UriListErrorKind;

    #[test]
    fn gnome_formats_copy_and_cut_with_uri_list() {
        let paths = [
            b"/home/nico/mi carpeta/a.txt".as_slice(),
            b"/tmp/b".as_slice(),
        ];
        for cut in [false, true] {
            let payload = encode_gnome_copied_files(&paths, cut).unwrap();
            assert!(!payload.ends_with(b"\n"));
            let expected_action = if cut { b"cut\n".as_slice() } else { b"copy\n" };
            assert!(payload.starts_with(expected_action));
            assert_eq!(
                decode_gnome_copied_files(&payload).unwrap(),
                (paths.map(|path| path.to_vec()).to_vec(), cut)
            );
        }
    }

    #[test]
    fn gnome_rejects_unknown_action_and_empty_file_list() {
        assert_eq!(
            decode_gnome_copied_files(b"move\nfile:///tmp/a"),
            Err(UriListError {
                line: 1,
                kind: UriListErrorKind::UnsupportedAction
            })
        );
        assert_eq!(
            decode_gnome_copied_files(b"copy\n"),
            Err(UriListError {
                line: 2,
                kind: UriListErrorKind::MissingPath
            })
        );
    }

    #[test]
    fn kde_cutselection_round_trips_and_rejects_ambiguous_data() {
        assert_eq!(
            decode_kde_cutselection(&encode_kde_cutselection(false)),
            Some(false)
        );
        assert_eq!(
            decode_kde_cutselection(&encode_kde_cutselection(true)),
            Some(true)
        );
        assert_eq!(decode_kde_cutselection(b""), None);
        assert_eq!(decode_kde_cutselection(b"10"), None);
    }
}
