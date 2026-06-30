// Naygo — comprimir/extraer .zip: lógica pura (sin UI ni Windows), cancelable y testeable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Crear y extraer archivos `.zip`. Puro: recibe rutas + un `CancellationToken` + callbacks de
//! progreso/conflicto; no conoce egui/Slint/Win32. El crate `zip` ya es dependencia (lo usa el
//! preview). Protección zip-slip al extraer (mismo criterio que `icon_pack::import`).

use crate::cancel::CancellationToken;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

/// Resultado de UNA entrada procesada (para el resumen y el deshacer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveOpItem {
    /// Ruta de la entrada procesada. Al COMPRIMIR es el ORIGEN que se metió al .zip (NO el .zip:
    /// el .zip es la única ruta creada, pero su undo se arma aparte desde el destino). Al EXTRAER
    /// es el archivo/carpeta de destino escrito.
    pub path: PathBuf,
    pub outcome: ArchiveOutcome,
    /// `true` solo si la operación CREÓ esta ruta de cero (no existía antes). Es la única base
    /// segura para deshacer: trashear al deshacer SOLO lo que la op trajo a la existencia, nunca
    /// algo preexistente del usuario. Para comprimir, los `path` son los ORÍGENES (que ya existían):
    /// `created` es `false` en todos (el undo de comprimir trashea el .zip, no los items — ver
    /// `compress_zip`). Para extraer: `true` solo en dirs que NO existían, archivos nuevos y la
    /// ruta con sufijo "(2)" de KeepBoth; `false` en dirs/archivos preexistentes (Overwrite incluido)
    /// y en lo Saltado/Fallido.
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveOutcome {
    Done,
    Skipped,
    Failed(String),
}

/// Decisión ante un conflicto al extraer (un archivo destino ya existe).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractConflict {
    Overwrite,
    Skip,
    KeepBoth,
    Cancel,
}

/// Error tipado de las operaciones de archivo comprimido. Ningún panic.
#[derive(Debug)]
pub enum ArchiveError {
    Io(std::io::Error),
    Zip(String),
    Cancelled,
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Io(e) => write!(f, "io: {e}"),
            ArchiveError::Zip(s) => write!(f, "zip: {s}"),
            ArchiveError::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl From<std::io::Error> for ArchiveError {
    fn from(e: std::io::Error) -> Self {
        ArchiveError::Io(e)
    }
}

/// Nombre por defecto del `.zip` a crear desde `sources`:
/// - 1 ítem (archivo o carpeta) → su nombre base + ".zip".
/// - varios ítems → "archivos.zip".
///
/// El nombre base de un archivo CONSERVA su parte sin extensión: "informe.txt" → "informe.zip".
/// Una carpeta usa su nombre tal cual: "proyecto" → "proyecto.zip".
pub fn default_zip_name(sources: &[PathBuf]) -> String {
    if sources.len() == 1 {
        let p = &sources[0];
        // file_stem para archivos con extensión; para carpetas (sin "extensión") file_stem == nombre.
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            return format!("{stem}.zip");
        }
    }
    "archivos.zip".to_string()
}

/// Recorre `sources` (archivos y/o carpetas, recursivo) y los empaqueta en `dest_zip`.
/// Progreso por BYTES. Cancelar → aborta y borra el `.zip` parcial. Un source ilegible se
/// registra Failed y la op continúa.
pub fn compress_zip(
    sources: &[PathBuf],
    dest_zip: &Path,
    on_progress: &mut dyn FnMut(u64, u64),
    token: &CancellationToken,
) -> Result<Vec<ArchiveOpItem>, ArchiveError> {
    let mut entries: Vec<(PathBuf, String)> = Vec::new();
    for src in sources {
        let base = src.parent().unwrap_or(Path::new(""));
        collect_entries(src, base, &mut entries);
    }
    let total: u64 = entries
        .iter()
        .map(|(p, _)| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();

    let file = std::fs::File::create(dest_zip)?;
    let mut zipw = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut done: u64 = 0;
    let mut items: Vec<ArchiveOpItem> = Vec::new();
    for (disk, internal) in &entries {
        if token.is_cancelled() {
            drop(zipw);
            let _ = std::fs::remove_file(dest_zip);
            return Err(ArchiveError::Cancelled);
        }
        token.wait_if_paused();
        if disk.is_dir() {
            let dir_name = format!("{}/", internal.trim_end_matches('/'));
            if let Err(e) = zipw.add_directory(dir_name, opts) {
                items.push(ArchiveOpItem {
                    path: disk.clone(),
                    outcome: ArchiveOutcome::Failed(e.to_string()),
                    // El `path` es un ORIGEN preexistente: comprimir NUNCA lo crea (la única ruta
                    // creada es el .zip). `created: false` siempre, para no trashear orígenes al
                    // deshacer.
                    created: false,
                });
            }
            continue;
        }
        match std::fs::File::open(disk) {
            Ok(mut f) => {
                if let Err(e) = zipw.start_file(internal.clone(), opts) {
                    items.push(ArchiveOpItem {
                        path: disk.clone(),
                        outcome: ArchiveOutcome::Failed(e.to_string()),
                        created: false,
                    });
                    continue;
                }
                let mut buf = [0u8; 64 * 1024];
                let mut failed = false;
                loop {
                    if token.is_cancelled() {
                        drop(zipw);
                        let _ = std::fs::remove_file(dest_zip);
                        return Err(ArchiveError::Cancelled);
                    }
                    let n = match f.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => {
                            failed = true;
                            break;
                        }
                    };
                    if zipw.write_all(&buf[..n]).is_err() {
                        failed = true;
                        break;
                    }
                    done += n as u64;
                    // Acota a `total`: si un archivo crece entre el escaneo de metadata y la
                    // lectura, `done` podría superar `total` (barra >100%).
                    on_progress(done.min(total), total);
                }
                items.push(ArchiveOpItem {
                    path: disk.clone(),
                    outcome: if failed {
                        ArchiveOutcome::Failed("read/write".into())
                    } else {
                        ArchiveOutcome::Done
                    },
                    created: false,
                });
            }
            Err(e) => items.push(ArchiveOpItem {
                path: disk.clone(),
                outcome: ArchiveOutcome::Failed(e.to_string()),
                created: false,
            }),
        }
    }
    zipw.finish()
        .map_err(|e| ArchiveError::Zip(e.to_string()))?;
    Ok(items)
}

/// Expande `path` a entradas (disk, internal). `base` es la carpeta padre del source de nivel
/// superior; la ruta interna en el zip es `path` relativa a `base` (con `/` como separador).
fn collect_entries(path: &Path, base: &Path, out: &mut Vec<(PathBuf, String)>) {
    // Ruta interna en el zip = `path` relativa a `base`. Si `strip_prefix` falla (p. ej. un
    // source que es la raíz de un disco "C:\", sin parent → base ""), usamos el nombre del
    // archivo: nunca dejamos una ruta ABSOLUTA dentro del zip (quedaría malformado).
    let internal = path
        .strip_prefix(base)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    if path.is_dir() {
        out.push((path.to_path_buf(), internal));
        if let Ok(rd) = std::fs::read_dir(path) {
            let mut children: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            children.sort();
            for c in children {
                collect_entries(&c, base, out);
            }
        }
    } else {
        out.push((path.to_path_buf(), internal));
    }
}

/// Extrae `zip` dentro de `dest_dir`. Zip-slip: entradas con `..`/absolutas o que escapen de
/// `dest_dir` se RECHAZAN (Skipped). Progreso por bytes (total = suma de tamaños descomprimidos).
/// Conflicto (destino existe) → `on_conflict`. Cancelar → aborta; lo extraído PERMANECE.
pub fn extract_zip(
    zip: &Path,
    dest_dir: &Path,
    on_conflict: &mut dyn FnMut(&Path) -> ExtractConflict,
    on_progress: &mut dyn FnMut(u64, u64),
    token: &CancellationToken,
) -> Result<Vec<ArchiveOpItem>, ArchiveError> {
    let file = std::fs::File::open(zip)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| ArchiveError::Zip(e.to_string()))?;
    let total: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.size()))
        .sum();
    std::fs::create_dir_all(dest_dir)?;
    let dest_canon = std::fs::canonicalize(dest_dir).unwrap_or_else(|_| dest_dir.to_path_buf());

    let mut done: u64 = 0;
    let mut items: Vec<ArchiveOpItem> = Vec::new();
    for i in 0..archive.len() {
        if token.is_cancelled() {
            return Err(ArchiveError::Cancelled);
        }
        token.wait_if_paused();
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                items.push(ArchiveOpItem {
                    path: PathBuf::new(),
                    outcome: ArchiveOutcome::Failed(e.to_string()),
                    created: false, // no se creó nada (no se pudo ni leer la entrada)
                });
                continue;
            }
        };
        // `enclosed_name` ya neutraliza `..` y rutas absolutas; si es None, es una entrada hostil.
        let Some(rel) = entry.enclosed_name() else {
            items.push(ArchiveOpItem {
                path: PathBuf::from(entry.name()),
                outcome: ArchiveOutcome::Skipped,
                created: false, // entrada hostil rechazada: nada en disco
            });
            continue;
        };
        let target = dest_dir.join(&rel);
        let within = target.starts_with(dest_dir)
            || std::fs::canonicalize(target.parent().unwrap_or(dest_dir))
                .map(|p| p.starts_with(&dest_canon))
                .unwrap_or(false);
        if !within {
            items.push(ArchiveOpItem {
                path: target,
                outcome: ArchiveOutcome::Skipped,
                created: false, // zip-slip bloqueado: nada se escribió
            });
            continue;
        }
        if entry.is_dir() {
            // CRÍTICO para el undo (BUG 2): solo es una ruta CREADA por la op si la carpeta NO
            // existía antes. Si el usuario ya tenía esta carpeta (posiblemente poblada), `created`
            // queda en `false` para que deshacer NUNCA la mande a la papelera con su contenido.
            let existed = target.exists();
            let _ = std::fs::create_dir_all(&target);
            items.push(ArchiveOpItem {
                path: target,
                outcome: ArchiveOutcome::Done,
                created: !existed,
            });
            continue;
        }
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut final_target = target.clone();
        // ¿La ruta destino del archivo CREADA por esta op es nueva? Arranca asumiendo que el
        // archivo no existía (extracción limpia → ruta nueva → trasheable al deshacer). El conflicto
        // la baja a `false` si vamos a PISAR un archivo preexistente (Overwrite): ese archivo ya
        // existía, así que deshacer NO debe trashearlo (su versión original se perdió al sobrescribir
        // — la sobrescritura no es deshacible de forma segura). KeepBoth escribe en una ruta "(2)"
        // que SÍ es nueva, así que vuelve a `true`.
        let mut created = true;
        if final_target.exists() {
            match on_conflict(&final_target) {
                ExtractConflict::Skip => {
                    items.push(ArchiveOpItem {
                        path: final_target,
                        outcome: ArchiveOutcome::Skipped,
                        created: false, // no se pisó nada: el preexistente quedó intacto
                    });
                    continue;
                }
                ExtractConflict::Cancel => return Err(ArchiveError::Cancelled),
                // Overwrite: el archivo YA existía. No es una ruta creada por la op → no trasheable.
                ExtractConflict::Overwrite => created = false,
                ExtractConflict::KeepBoth => {
                    // La ruta con sufijo "(2)" es nueva (creada por la op) → trasheable.
                    final_target = unique_path(&final_target);
                    created = true;
                }
            }
        }
        match std::fs::File::create(&final_target) {
            Ok(mut out) => {
                let mut buf = [0u8; 64 * 1024];
                // Captura el error real del OS (lectura del zip o escritura del disco) para el
                // resumen, en vez de un genérico "write". `None` = se copió completo.
                let mut fail_msg: Option<String> = None;
                loop {
                    if token.is_cancelled() {
                        return Err(ArchiveError::Cancelled);
                    }
                    let n = match entry.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(e) => {
                            fail_msg = Some(e.to_string());
                            break;
                        }
                    };
                    if let Err(e) = out.write_all(&buf[..n]) {
                        fail_msg = Some(e.to_string());
                        break;
                    }
                    done += n as u64;
                    // Acota a `total`: un zip con `size` declarado menor al real haría done > total
                    // (barra >100%). Nunca reportamos progreso por encima del total.
                    on_progress(done.min(total), total);
                }
                items.push(ArchiveOpItem {
                    path: final_target,
                    outcome: match fail_msg {
                        Some(m) => ArchiveOutcome::Failed(m),
                        None => ArchiveOutcome::Done,
                    },
                    // `created` solo importa para los Done (el undo solo trashea Done); un Overwrite
                    // sobre preexistente lo dejó en `false` para no trashear lo que ya existía.
                    created,
                });
            }
            Err(e) => items.push(ArchiveOpItem {
                path: final_target,
                outcome: ArchiveOutcome::Failed(e.to_string()),
                created: false, // no se pudo crear el archivo: nada en disco que deshacer
            }),
        }
    }
    Ok(items)
}

/// Nombre de `.zip` LIBRE (que no pisa un archivo existente) dentro de `dir`. Si `dir/name` no
/// existe, devuelve `name` tal cual; si existe, desambigua con el primer sufijo "(N)" libre:
/// "proyecto.zip" ocupado → "proyecto (2).zip" → "proyecto (3).zip"…
///
/// CRÍTICO (pérdida de datos): al COMPRIMIR, el `.zip` se crea con `File::create`, que TRUNCA un
/// archivo del mismo nombre sin avisar y sin pasar por la papelera. El controlador desambigua el
/// nombre con este helper ANTES de armar la op, de modo que `compress_zip` nunca pise un `.zip`
/// preexistente del usuario. Devuelve solo el NOMBRE (no la ruta) porque la op de comprimir lleva
/// `dest_name` + `dest_dir` por separado; así el `dest_name` desambiguado fluye al worker y al undo
/// (el `TrashCreated` del deshacer apunta al `.zip` realmente creado, no al nombre original).
pub fn unique_zip_name(dir: &Path, name: &str) -> String {
    let unique = unique_path(&dir.join(name));
    unique
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string())
}

/// Devuelve una variante de `path` que no existe: "a.txt" → "a (2).txt" → "a (3).txt"…
fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("archivo");
    let ext = path.extension().and_then(|s| s.to_str());
    for n in 2..10_000 {
        let name = match ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let cand = parent.join(name);
        if !cand.exists() {
            return cand;
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_zip_name_un_archivo_usa_su_stem() {
        assert_eq!(
            default_zip_name(&[PathBuf::from("C:/x/informe.txt")]),
            "informe.zip"
        );
    }

    #[test]
    fn default_zip_name_una_carpeta_usa_su_nombre() {
        assert_eq!(
            default_zip_name(&[PathBuf::from("C:/x/proyecto")]),
            "proyecto.zip"
        );
    }

    #[test]
    fn default_zip_name_varios_es_generico() {
        let v = vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")];
        assert_eq!(default_zip_name(&v), "archivos.zip");
    }

    fn noop_progress() -> impl FnMut(u64, u64) {
        |_done, _total| {}
    }

    #[test]
    fn unique_zip_name_no_existe_devuelve_tal_cual() {
        let dir = tempfile::tempdir().unwrap();
        // No hay "foo.zip" en `dir`: el nombre se devuelve sin tocar.
        assert_eq!(unique_zip_name(dir.path(), "foo.zip"), "foo.zip");
    }

    #[test]
    fn unique_zip_name_existe_desambigua_con_sufijo() {
        let dir = tempfile::tempdir().unwrap();
        // Ya existe "foo.zip": debe saltar a "foo (2).zip".
        std::fs::write(dir.path().join("foo.zip"), b"viejo").unwrap();
        assert_eq!(unique_zip_name(dir.path(), "foo.zip"), "foo (2).zip");
        // Con "foo.zip" Y "foo (2).zip" ocupados, sube a "foo (3).zip".
        std::fs::write(dir.path().join("foo (2).zip"), b"viejo2").unwrap();
        assert_eq!(unique_zip_name(dir.path(), "foo.zip"), "foo (3).zip");
    }

    #[test]
    fn compress_no_pisa_un_zip_existente_desambiguando() {
        // Pérdida de datos: comprimir con un `dest_name` que YA existe NO debe truncar el .zip
        // preexistente. El flujo desambigua el nombre con `unique_zip_name` ANTES de comprimir,
        // así `compress_zip` escribe en "foo (2).zip" y el "foo.zip" original queda intacto.
        let dir = tempfile::tempdir().unwrap();
        // El usuario ya tiene un "foo.zip" propio (de una compresión anterior).
        let preexistente = dir.path().join("foo.zip");
        std::fs::write(&preexistente, b"contenido-original-irreemplazable").unwrap();
        // Fuente nueva a comprimir.
        let src = dir.path().join("nuevo.txt");
        std::fs::write(&src, b"datos nuevos").unwrap();

        // El controlador desambigua el nombre destino ANTES de comprimir.
        let dest_name = unique_zip_name(dir.path(), "foo.zip");
        assert_eq!(dest_name, "foo (2).zip");
        let dest_zip = dir.path().join(&dest_name);

        let token = CancellationToken::new();
        compress_zip(&[src], &dest_zip, &mut noop_progress(), &token).unwrap();

        // El "foo.zip" original NO fue tocado (mismo contenido byte a byte).
        assert_eq!(
            std::fs::read(&preexistente).unwrap(),
            b"contenido-original-irreemplazable",
            "el .zip preexistente NO debe ser pisado"
        );
        // El nuevo "foo (2).zip" existe y trae la fuente comprimida.
        assert!(dest_zip.exists(), "se creó el .zip desambiguado");
        let f = std::fs::File::open(&dest_zip).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        assert!(
            z.by_name("nuevo.txt").is_ok(),
            "el .zip nuevo trae la fuente"
        );
    }

    #[test]
    fn compress_un_archivo_crea_zip_leible() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("hola.txt");
        std::fs::write(&src, b"contenido").unwrap();
        let zip_path = dir.path().join("out.zip");
        let token = CancellationToken::new();
        let items = compress_zip(
            std::slice::from_ref(&src),
            &zip_path,
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        assert!(zip_path.exists());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].outcome, ArchiveOutcome::Done);
        let f = std::fs::File::open(&zip_path).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        assert!(z.by_name("hola.txt").is_ok());
    }

    #[test]
    fn compress_carpeta_anidada_incluye_todo_recursivo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proyecto");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("README.md"), b"x").unwrap();
        std::fs::write(root.join("src/main.rs"), b"fn main(){}").unwrap();
        let zip_path = dir.path().join("p.zip");
        let token = CancellationToken::new();
        compress_zip(
            std::slice::from_ref(&root),
            &zip_path,
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        let f = std::fs::File::open(&zip_path).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        let names: Vec<String> = (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n.ends_with("proyecto/README.md")));
        assert!(names.iter().any(|n| n.ends_with("proyecto/src/main.rs")));
    }

    #[test]
    fn compress_cancelado_borra_el_parcial() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        std::fs::write(&src, vec![0u8; 1024]).unwrap();
        let zip_path = dir.path().join("c.zip");
        let token = CancellationToken::new();
        token.cancel();
        let r = compress_zip(&[src], &zip_path, &mut noop_progress(), &token);
        assert!(matches!(r, Err(ArchiveError::Cancelled)));
        assert!(!zip_path.exists(), "el .zip parcial se borra al cancelar");
    }

    #[test]
    fn compress_cancelado_a_media_copia_borra_el_parcial() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("grande.bin");
        std::fs::write(&src, vec![7u8; 4 * 1024 * 1024]).unwrap(); // 4 MB > 64KB
        let zip_path = dir.path().join("c.zip");
        let token = CancellationToken::new();
        // Cancelar tras un instante, desde otro hilo, para pegarle a media copia.
        let t2 = token.clone();
        let h = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1));
            t2.cancel();
        });
        let r = compress_zip(&[src], &zip_path, &mut noop_progress(), &token);
        h.join().unwrap();
        // Puede alcanzar a terminar antes de cancelar (archivo chico para el disco); si canceló,
        // el parcial NO debe quedar. Aceptamos ambos finales pero sin .zip a medias.
        if matches!(r, Err(ArchiveError::Cancelled)) {
            assert!(
                !zip_path.exists(),
                "cancelado a media copia: parcial borrado"
            );
        }
    }

    fn always_overwrite() -> impl FnMut(&Path) -> ExtractConflict {
        |_p| ExtractConflict::Overwrite
    }

    /// Crea un .zip de prueba con las entradas dadas (nombre interno → contenido). dir-entries
    /// terminan en '/'. Devuelve la ruta del zip.
    fn make_zip(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let zip_path = dir.join(name);
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        for (n, data) in entries {
            if n.ends_with('/') {
                z.add_directory(*n, opts).unwrap();
            } else {
                z.start_file(*n, opts).unwrap();
                z.write_all(data).unwrap();
            }
        }
        z.finish().unwrap();
        zip_path
    }

    #[test]
    fn extract_round_trip_restaura_los_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "in.zip", &[("a/b.txt", b"hola"), ("a/", b"")]);
        let dest = dir.path().join("salida");
        let token = CancellationToken::new();
        let items = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        assert!(items.iter().any(|i| i.outcome == ArchiveOutcome::Done));
        assert_eq!(std::fs::read(dest.join("a/b.txt")).unwrap(), b"hola");
    }

    #[test]
    fn extract_rechaza_zip_slip() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(
            dir.path(),
            "evil.zip",
            &[("../escape.txt", b"pwn"), ("ok.txt", b"bien")],
        );
        let dest = dir.path().join("destino");
        let token = CancellationToken::new();
        let items = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        assert_eq!(std::fs::read(dest.join("ok.txt")).unwrap(), b"bien");
        assert!(
            !dir.path().join("escape.txt").exists(),
            "zip-slip bloqueado: nada fuera del destino"
        );
        assert!(items.iter().any(|i| i.outcome == ArchiveOutcome::Skipped));
    }

    #[test]
    fn extract_conflicto_skip_no_pisa() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("dato.txt", b"nuevo")]);
        let dest = dir.path().join("d");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("dato.txt"), b"viejo").unwrap();
        let token = CancellationToken::new();
        let mut skip = |_p: &Path| ExtractConflict::Skip;
        extract_zip(&zip_path, &dest, &mut skip, &mut noop_progress(), &token).unwrap();
        assert_eq!(
            std::fs::read(dest.join("dato.txt")).unwrap(),
            b"viejo",
            "Skip no pisa el existente"
        );
    }

    #[test]
    fn extract_conflicto_keepboth_renombra() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("dato.txt", b"nuevo")]);
        let dest = dir.path().join("d");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("dato.txt"), b"viejo").unwrap();
        let token = CancellationToken::new();
        let mut keep = |_p: &Path| ExtractConflict::KeepBoth;
        extract_zip(&zip_path, &dest, &mut keep, &mut noop_progress(), &token).unwrap();
        // El existente se conserva y el nuevo va con sufijo.
        assert_eq!(std::fs::read(dest.join("dato.txt")).unwrap(), b"viejo");
        assert_eq!(std::fs::read(dest.join("dato (2).txt")).unwrap(), b"nuevo");
    }

    #[test]
    fn extract_cancelado_deja_lo_ya_extraido() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("uno.txt", b"a")]);
        let dest = dir.path().join("d");
        let token = CancellationToken::new();
        token.cancel();
        let r = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        );
        assert!(matches!(r, Err(ArchiveError::Cancelled)));
    }

    /// Una entrada con datos comprimidos corruptos NO debe abortar la extracción: la entrada
    /// dañada queda Failed y las sanas siguen extrayéndose.
    ///
    /// Enfoque determinista: se crea un .zip deflated con dos entradas; la PRIMERA con contenido
    /// muy comprimible (su stream deflate es válido y verificable por CRC). Luego se corrompen
    /// unos bytes en la región de datos de esa primera entrada — justo después de su cabecera
    /// local (Local File Header) — dejando intactos el resto del archivo y el directorio central
    /// al final, para que `ZipArchive::new` abra bien pero `read()` falle (inflate/CRC) en la
    /// entrada dañada. La cabecera local mide 30 bytes + nombre (sin extra field por defecto).
    #[test]
    fn extract_entrada_corrupta_falla_sin_abortar() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("danado.zip");
        // Crear el zip deflated a mano para controlar el método de compresión.
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut z = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            // Contenido repetitivo → el stream deflate es no trivial y su CRC valida bien.
            z.start_file("malo.txt", opts).unwrap();
            z.write_all(&vec![b'A'; 4096]).unwrap();
            z.start_file("bueno.txt", opts).unwrap();
            z.write_all(&vec![b'B'; 4096]).unwrap();
            z.finish().unwrap();
        }
        // Corromper bytes en la región de datos de "malo.txt": tras su Local File Header.
        // LFH = 30 bytes fijos + longitud del nombre ("malo.txt" = 8) = 38 bytes de offset 0.
        let mut bytes = std::fs::read(&zip_path).unwrap();
        let data_off = 30 + "malo.txt".len();
        for b in bytes.iter_mut().skip(data_off).take(16) {
            *b ^= 0xFF;
        }
        std::fs::write(&zip_path, &bytes).unwrap();

        let dest = dir.path().join("out");
        let token = CancellationToken::new();
        // No debe entrar en pánico ni abortar: devuelve Ok con el detalle por entrada.
        let items = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        )
        .expect("entrada corrupta no aborta la operación");
        // La entrada sana se extrajo bien.
        assert!(
            items.iter().any(|i| i.outcome == ArchiveOutcome::Done),
            "la entrada sana debe extraerse (Done)"
        );
        assert_eq!(
            std::fs::read(dest.join("bueno.txt")).unwrap(),
            vec![b'B'; 4096]
        );
        // La entrada dañada quedó marcada Failed (no Done).
        assert!(
            items
                .iter()
                .any(|i| matches!(i.outcome, ArchiveOutcome::Failed(_))),
            "la entrada corrupta debe quedar Failed: {items:?}"
        );
    }

    // --- Regresión: el flag `created` para el undo seguro (no perder datos) ---

    #[test]
    fn compress_items_apuntan_a_origenes_y_nunca_son_created() {
        // BUG 1 (pérdida de datos): comprimir reportaba cada ORIGEN como item Done; si el undo
        // trasheara esos items, borraría los archivos del usuario y dejaría el .zip. La defensa es
        // doble: (a) los `path` son los orígenes (no el .zip), (b) `created` es SIEMPRE false, así
        // que `zip_undo_actions` no trashea ninguno (el undo de comprimir apunta al .zip aparte).
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("hola.txt");
        std::fs::write(&src, b"contenido").unwrap();
        let zip_path = dir.path().join("out.zip");
        let token = CancellationToken::new();
        let items = compress_zip(
            std::slice::from_ref(&src),
            &zip_path,
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, src, "el item apunta al ORIGEN, no al .zip");
        assert_ne!(items[0].path, zip_path, "el .zip NO aparece como item");
        assert!(
            !items.iter().any(|i| i.created),
            "NINGÚN item de comprimir es 'created' (no trashear orígenes al deshacer)"
        );
    }

    #[test]
    fn extract_dir_preexistente_no_es_created() {
        // BUG 2 (pérdida de datos): extraer un zip que trae una entrada de carpeta cuyo nombre el
        // usuario YA tenía (poblada). Esa carpeta NO la creó la op → created=false, para que
        // deshacer no la mande a la papelera con el contenido previo del usuario.
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(
            dir.path(),
            "z.zip",
            &[("sub/", b""), ("sub/nuevo.txt", b"x")],
        );
        let dest = dir.path().join("d");
        // El usuario ya tenía `d/sub/` con un archivo propio.
        std::fs::create_dir_all(dest.join("sub")).unwrap();
        std::fs::write(dest.join("sub/previo.txt"), b"mio").unwrap();
        let token = CancellationToken::new();
        let items = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        // La entrada de carpeta `sub/` apunta a la carpeta preexistente: created=false.
        let sub_item = items
            .iter()
            .find(|i| i.path == dest.join("sub"))
            .expect("debe haber un item para la carpeta sub/");
        assert_eq!(sub_item.outcome, ArchiveOutcome::Done);
        assert!(
            !sub_item.created,
            "carpeta preexistente NO es created (no trashear el contenido del usuario)"
        );
        // El archivo previo del usuario sigue intacto.
        assert_eq!(std::fs::read(dest.join("sub/previo.txt")).unwrap(), b"mio");
    }

    #[test]
    fn extract_dir_nuevo_si_es_created() {
        // Una carpeta que NO existía antes SÍ la creó la op → created=true (trasheable al deshacer).
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(
            dir.path(),
            "z.zip",
            &[("nueva/", b""), ("nueva/a.txt", b"x")],
        );
        let dest = dir.path().join("d"); // d/ no tiene "nueva/" todavía
        let token = CancellationToken::new();
        let items = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        let nueva = items
            .iter()
            .find(|i| i.path == dest.join("nueva"))
            .expect("debe haber un item para la carpeta nueva/");
        assert!(
            nueva.created,
            "carpeta nueva creada por la op → created=true"
        );
        // El archivo dentro también es nuevo.
        let archivo = items
            .iter()
            .find(|i| i.path == dest.join("nueva/a.txt"))
            .unwrap();
        assert!(archivo.created, "archivo nuevo → created=true");
    }

    #[test]
    fn extract_archivo_nuevo_si_es_created() {
        // Un archivo extraído en una ruta que no existía → created=true (trasheable).
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("dato.txt", b"nuevo")]);
        let dest = dir.path().join("d");
        let token = CancellationToken::new();
        let items = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        let item = items
            .iter()
            .find(|i| i.path == dest.join("dato.txt"))
            .unwrap();
        assert_eq!(item.outcome, ArchiveOutcome::Done);
        assert!(item.created, "archivo nuevo → created=true");
    }

    #[test]
    fn extract_overwrite_de_preexistente_no_es_created() {
        // BUG 2 (parte archivo): Overwrite sobre un archivo que el usuario YA tenía. Ese archivo
        // existía antes; deshacer NO debe trashearlo (su versión original ya se perdió al pisar).
        // created=false lo deja fuera del undo.
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("dato.txt", b"nuevo")]);
        let dest = dir.path().join("d");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("dato.txt"), b"viejo").unwrap();
        let token = CancellationToken::new();
        let items = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        let item = items
            .iter()
            .find(|i| i.path == dest.join("dato.txt"))
            .unwrap();
        assert_eq!(item.outcome, ArchiveOutcome::Done, "se sobrescribió (Done)");
        assert!(
            !item.created,
            "Overwrite de preexistente NO es created (no trashear lo que ya existía)"
        );
    }

    #[test]
    fn extract_keepboth_ruta_con_sufijo_es_created() {
        // KeepBoth ante un preexistente: escribe en "dato (2).txt", una ruta NUEVA creada por la op
        // → created=true (trasheable). El preexistente "dato.txt" no aparece como item creado.
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("dato.txt", b"nuevo")]);
        let dest = dir.path().join("d");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("dato.txt"), b"viejo").unwrap();
        let token = CancellationToken::new();
        let mut keep = |_p: &Path| ExtractConflict::KeepBoth;
        let items = extract_zip(&zip_path, &dest, &mut keep, &mut noop_progress(), &token).unwrap();
        // El item es la ruta con sufijo y es created=true.
        let item = items
            .iter()
            .find(|i| i.path == dest.join("dato (2).txt"))
            .expect("KeepBoth escribe en la ruta con sufijo");
        assert!(item.created, "la ruta '(2)' es nueva → created=true");
        // El preexistente intacto y NO aparece como ruta creada.
        assert_eq!(std::fs::read(dest.join("dato.txt")).unwrap(), b"viejo");
        assert!(!items
            .iter()
            .any(|i| i.path == dest.join("dato.txt") && i.created));
    }

    #[test]
    fn extract_skip_no_es_created() {
        // Skip ante un preexistente: no se escribe nada → created=false (no trashear el del usuario).
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("dato.txt", b"nuevo")]);
        let dest = dir.path().join("d");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("dato.txt"), b"viejo").unwrap();
        let token = CancellationToken::new();
        let mut skip = |_p: &Path| ExtractConflict::Skip;
        let items = extract_zip(&zip_path, &dest, &mut skip, &mut noop_progress(), &token).unwrap();
        let item = items
            .iter()
            .find(|i| i.path == dest.join("dato.txt"))
            .unwrap();
        assert_eq!(item.outcome, ArchiveOutcome::Skipped);
        assert!(!item.created, "Skip no crea nada → created=false");
    }

    #[test]
    fn extract_zip_slip_no_es_created() {
        // Una entrada zip-slip rechazada nunca toca el disco → created=false.
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "evil.zip", &[("../escape.txt", b"pwn")]);
        let dest = dir.path().join("destino");
        let token = CancellationToken::new();
        let items = extract_zip(
            &zip_path,
            &dest,
            &mut always_overwrite(),
            &mut noop_progress(),
            &token,
        )
        .unwrap();
        assert!(
            !items.iter().any(|i| i.created),
            "una entrada rechazada por zip-slip no es created"
        );
    }
}
