// Naygo — renombrado de archivos sin reemplazo, aislado por plataforma.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Implementación de plataforma para el movimiento rápido de archivos.
//! Windows usa `MoveFileExW` sin flags de reemplazo ni copia entre volúmenes, para que la
//! publicación falle atómicamente si otro proceso ocupa el destino. El rename de mismo volumen
//! falla ante EXDEV/ERROR_NOT_SAME_DEVICE y deja el fallback cancelable
//! de copia y retirada del origen a cargo del motor de `core`.

use std::path::Path;

/// Adaptador de `core` que aplica el rename no-clobber nativo de esta plataforma.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlatformNoReplaceRename;

impl naygo_core::ops::NoReplaceRename for PlatformNoReplaceRename {
    fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            rename_noreplace_windows(from, to)
        }
        #[cfg(not(windows))]
        {
            naygo_core::ops::NativeNoReplaceRename.rename_noreplace(from, to)
        }
    }
}

#[cfg(windows)]
fn rename_noreplace_windows(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVE_FILE_FLAGS};

    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();

    // Flags 0 omite MOVEFILE_REPLACE_EXISTING (no pisa un destino concurrente) y
    // MOVEFILE_COPY_ALLOWED (ERROR_NOT_SAME_DEVICE permite el fallback cancelable del core).
    // SAFETY: ambas rutas son wide strings terminadas en NUL y permanecen vivas durante la llamada.
    unsafe {
        MoveFileExW(
            PCWSTR(from_wide.as_ptr()),
            PCWSTR(to_wide.as_ptr()),
            MOVE_FILE_FLAGS(0),
        )
    }
    .map_err(|error| {
        let error = windows_error_to_io(error);
        std::io::Error::new(
            error.kind(),
            format!("no se pudo renombrar sin reemplazar: {error}"),
        )
    })?;

    Ok(())
}

#[cfg(windows)]
fn windows_error_to_io(error: windows::core::Error) -> std::io::Error {
    // Win32 APIs return HRESULT_FROM_WIN32(GetLastError()); keep the underlying Win32 code so
    // `ERROR_NOT_SAME_DEVICE` is recognized by core as CrossesDevices and triggers its fallback.
    let code = error.code().0 as u32;
    let win32_code = if code & 0xFFFF_0000 == 0x8007_0000 {
        (code & 0xFFFF) as i32
    } else {
        error.code().0
    };
    std::io::Error::from_raw_os_error(win32_code)
}

#[cfg(all(test, windows))]
mod tests {
    use super::{windows_error_to_io, PlatformNoReplaceRename};
    use naygo_core::ops::NoReplaceRename;
    use std::fs;

    #[test]
    fn error_hresult_not_same_device_activa_fallback_cross_volume() {
        use windows::core::{Error, HRESULT};

        // HRESULT_FROM_WIN32(ERROR_NOT_SAME_DEVICE) debe volver al código Win32 17,
        // reconocido por `core` como CrossesDevices para continuar con la copia cancelable.
        let error = windows_error_to_io(Error::from_hresult(HRESULT(0x8007_0011_u32 as i32)));

        assert_eq!(error.raw_os_error(), Some(17));
        assert_eq!(error.kind(), std::io::ErrorKind::CrossesDevices);
    }

    #[test]
    fn rename_atómico_rechaza_destino_existente_y_conserva_ambos_archivos() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("destination.txt");
        fs::write(&source, b"source").unwrap();
        fs::write(&destination, b"existing").unwrap();

        let error = PlatformNoReplaceRename
            .rename_noreplace(&source, &destination)
            .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(fs::read(&destination).unwrap(), b"existing");
    }

    #[test]
    fn rename_atómico_mueve_archivo_libre_en_el_mismo_volumen() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let destination_dir = dir.path().join("destination");
        fs::create_dir(&source_dir).unwrap();
        fs::create_dir(&destination_dir).unwrap();
        let source = std::path::PathBuf::from(format!("{}/source.txt", source_dir.display()));
        let destination =
            std::path::PathBuf::from(format!("{}/destination.txt", destination_dir.display()));
        fs::write(&source, b"payload").unwrap();

        PlatformNoReplaceRename
            .rename_noreplace(&source, &destination)
            .unwrap();

        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"payload");
    }

    #[test]
    fn rename_atómico_compite_con_creación_de_destino_sin_reemplazarlo() {
        use std::sync::{Arc, Barrier};

        // Cada iteración enfrenta el rename real de Win32 con create_dir sobre la misma ruta.
        // El ganador debe conservar su identidad y el perdedor no puede pisar ni retirar el otro.
        for iteration in 0..32 {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("source");
            let destination = dir.path().join("destination");
            fs::create_dir(&source).unwrap();
            fs::write(source.join("owner.txt"), b"rename").unwrap();

            let barrier = Arc::new(Barrier::new(2));
            let rename_barrier = Arc::clone(&barrier);
            let rename_source = source.clone();
            let rename_destination = destination.clone();
            let rename = std::thread::spawn(move || {
                rename_barrier.wait();
                PlatformNoReplaceRename.rename_noreplace(&rename_source, &rename_destination)
            });

            let create_barrier = Arc::clone(&barrier);
            let create_destination = destination.clone();
            let create = std::thread::spawn(move || {
                create_barrier.wait();
                fs::create_dir(create_destination)
            });

            let rename_result = rename.join().unwrap();
            let create_result = create.join().unwrap();
            match (rename_result, create_result) {
                (Ok(()), Err(error)) => {
                    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
                    assert!(!source.exists(), "iteración {iteration}: rename ganó");
                    assert_eq!(
                        fs::read(destination.join("owner.txt")).unwrap(),
                        b"rename",
                        "iteración {iteration}: el contenido movido permanece íntegro"
                    );
                }
                (Err(error), Ok(())) => {
                    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
                    assert_eq!(
                        fs::read(source.join("owner.txt")).unwrap(),
                        b"rename",
                        "iteración {iteration}: el origen permanece íntegro"
                    );
                    assert!(
                        fs::read_dir(&destination).unwrap().next().is_none(),
                        "iteración {iteration}: la carpeta creada por el competidor no se reemplaza"
                    );
                }
                (rename_result, create_result) => panic!(
                    "iteración {iteration}: exactamente una operación debe ganar; rename={rename_result:?}, create={create_result:?}"
                ),
            }
        }
    }
}
