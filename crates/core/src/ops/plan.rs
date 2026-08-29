// Naygo — planificación de operaciones: expandir a pasos + validar (recorre FS).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `plan` toma una `OpRequest` y produce un `OpPlan` (lista de pasos + totales),
//! validando precondiciones (nombres, carpeta-dentro-de-sí-misma). Para Copy/Move
//! recorre el árbol de orígenes leyendo tamaños. Devuelve `Result<OpPlan, PlanError>`.

use super::names::{is_valid_name, relative_components};
use super::{OpKind, OpPlan, OpRequest, OpStep};
use std::path::{Path, PathBuf};

/// Error de planificación (antes de empezar a ejecutar).
#[derive(Debug, Clone, PartialEq)]
pub enum PlanError {
    /// El destino está dentro de uno de los orígenes (copia recursiva infinita).
    DestInsideSource,
    /// Nombre inválido (al renombrar/crear).
    InvalidName(String),
    /// Falta el destino para una op que lo requiere.
    MissingDest,
    /// Un origen no existe / no se pudo leer.
    SourceUnreadable(PathBuf),
}

/// Planifica una `OpRequest`: produce los pasos + totales, o un `PlanError`.
pub fn plan(req: &OpRequest) -> Result<OpPlan, PlanError> {
    match &req.kind {
        OpKind::Copy | OpKind::Move => plan_transfer(req),
        OpKind::Duplicate => plan_duplicate(req),
        OpKind::Delete { .. } => {
            let mut sink = |_files: usize, _bytes: u64| {};
            plan_delete_with(req, &mut sink, &|| false)
        }
        OpKind::Rename { new_name } => {
            if !is_valid_name(new_name) {
                return Err(PlanError::InvalidName(new_name.clone()));
            }
            let from = req.sources.first().cloned();
            let to = from
                .as_ref()
                .and_then(|p| p.parent())
                .map(|parent| parent.join(new_name))
                .ok_or(PlanError::MissingDest)?;
            Ok(OpPlan {
                steps: vec![OpStep {
                    from,
                    to,
                    bytes: 0,
                    is_dir: false,
                }],
                total_bytes: 0,
                total_files: 1,
                pre_delete: Vec::new(),
            })
        }
        OpKind::BatchRename { new_names } => plan_batch_rename(req, new_names),
        OpKind::CreateDir { name } | OpKind::CreateFile { name } => {
            let dest = req.dest_dir.clone().ok_or(PlanError::MissingDest)?;
            let is_dir = matches!(req.kind, OpKind::CreateDir { .. });
            // El nombre puede ser una RUTA RELATIVA anidada (`a\b\c` o `a/b/c`): validamos
            // CADA componente con `is_valid_name` y rechazamos `.`/`..`/vacío/absoluta, en vez
            // de pasar el string completo por `is_valid_name` (que prohíbe los separadores).
            // El destino se arma uniendo componente a componente sobre `dest`, así NUNCA
            // puede escapar de la carpeta destino (no hay `..` ni rutas absolutas).
            let to = match relative_components(name) {
                Some(parts) => {
                    let mut to = dest.clone();
                    for part in parts {
                        to.push(part);
                    }
                    to
                }
                None => return Err(PlanError::InvalidName(name.clone())),
            };
            Ok(OpPlan {
                steps: vec![OpStep {
                    from: None,
                    to,
                    bytes: 0,
                    is_dir,
                }],
                total_bytes: 0,
                total_files: if is_dir { 0 } else { 1 },
                pre_delete: Vec::new(),
            })
        }
        // Compress/Extract NO pasan por este planificador: las maneja el worker de zip
        // (ui-slint) con su propia lógica. Si llegaran aquí, devolvemos un plan vacío
        // (sin pasos) en vez de inventar semántica del motor de copia.
        OpKind::Compress { .. } | OpKind::Extract => Ok(OpPlan {
            steps: Vec::new(),
            total_bytes: 0,
            total_files: 0,
            pre_delete: Vec::new(),
        }),
    }
}

/// Plan del renombrado en lote: un paso `from → parent(from)/new_name` por ítem,
/// ORDENADOS por dependencia — un paso cuyo destino está ocupado por el origen de
/// otro paso pendiente va después (shifts foto1→foto2, foto2→foto3 se resuelven
/// solos). Si no hay progreso (ciclo a↔b, no soportado en v1) → `InvalidName` del
/// destino atascado. El preview del diálogo ya bloquea estos casos; esto es la red
/// de seguridad para llamadas directas al motor.
fn plan_batch_rename(req: &OpRequest, new_names: &[String]) -> Result<OpPlan, PlanError> {
    if new_names.len() != req.sources.len() {
        return Err(PlanError::MissingDest);
    }
    for name in new_names {
        if !is_valid_name(name) {
            return Err(PlanError::InvalidName(name.clone()));
        }
    }
    let mut pending: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(req.sources.len());
    for (src, name) in req.sources.iter().zip(new_names) {
        if !src.exists() {
            return Err(PlanError::SourceUnreadable(src.clone()));
        }
        let to = src
            .parent()
            .map(|p| p.join(name))
            .ok_or(PlanError::MissingDest)?;
        pending.push((src.clone(), to));
    }
    // Clave case-insensitive (semántica de nombres de Windows).
    let key = |p: &Path| p.to_string_lossy().to_lowercase();
    let mut steps: Vec<OpStep> = Vec::with_capacity(pending.len());
    let mut freed: std::collections::HashSet<String> = std::collections::HashSet::new();
    loop {
        let mut progressed = false;
        pending.retain(|(from, to)| {
            // Puede correr si el destino está libre, lo liberó un paso ya agendado,
            // o es el propio origen (cambio solo de mayúsculas).
            let runnable = !to.exists() || freed.contains(&key(to)) || key(from) == key(to);
            if runnable {
                freed.insert(key(from));
                steps.push(OpStep {
                    from: Some(from.clone()),
                    to: to.clone(),
                    bytes: 0,
                    is_dir: false,
                });
                progressed = true;
                false
            } else {
                true
            }
        });
        if !progressed {
            break;
        }
    }
    if let Some((_, to)) = pending.first() {
        return Err(PlanError::InvalidName(
            to.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        ));
    }
    let n = steps.len();
    Ok(OpPlan {
        steps,
        total_bytes: 0,
        total_files: n,
        pre_delete: Vec::new(),
    })
}

fn plan_transfer(req: &OpRequest) -> Result<OpPlan, PlanError> {
    // Sin sink de progreso ni cancelación: el camino síncrono clásico.
    plan_transfer_with(req, &mut |_, _| {}, &|| false)
}

/// Planifica duplicados en la carpeta de cada origen. La resolución de nombres ocurre en el
/// worker de planificación (nunca en UI): `foto.png` → `foto - copia.png` →
/// `foto - copia (2).png`. El set reservado evita colisiones entre varios ítems del mismo lote.
fn plan_duplicate(req: &OpRequest) -> Result<OpPlan, PlanError> {
    plan_duplicate_with(req, &mut |_, _| {}, &|| false)
}

pub(super) fn plan_duplicate_with(
    req: &OpRequest,
    sink: &mut dyn FnMut(usize, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<OpPlan, PlanError> {
    let mut steps = Vec::new();
    let mut total_bytes = 0u64;
    let mut total_files = 0usize;
    let mut reserved = std::collections::HashSet::<String>::new();
    for src in &req.sources {
        if cancelled() {
            break;
        }
        let parent = src.parent().ok_or(PlanError::MissingDest)?;
        let name = src
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| PlanError::InvalidName(src.display().to_string()))?;
        let dest = unique_duplicate_path(parent, name, &reserved);
        reserved.insert(dest.to_string_lossy().to_lowercase());
        expand(
            src,
            &dest,
            &mut steps,
            &mut total_bytes,
            &mut total_files,
            sink,
            cancelled,
        )?;
    }
    Ok(OpPlan {
        steps,
        total_bytes,
        total_files,
        pre_delete: Vec::new(),
    })
}

fn unique_duplicate_path(
    parent: &Path,
    name: &str,
    reserved: &std::collections::HashSet<String>,
) -> PathBuf {
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let mut n = 1usize;
    loop {
        let suffix = if n == 1 {
            " - copia".to_string()
        } else {
            format!(" - copia ({n})")
        };
        let candidate = parent.join(format!("{stem}{suffix}{ext}"));
        if !candidate.exists() && !reserved.contains(&candidate.to_string_lossy().to_lowercase()) {
            return candidate;
        }
        n += 1;
    }
}

/// Núcleo de `plan_transfer` parametrizado por un `sink` de progreso y un predicado de
/// cancelación, para que el worker asíncrono (`plan_async::spawn_plan`) REUSE exactamente
/// este recorrido en vez de duplicarlo. El camino síncrono pasa un sink vacío y un
/// `cancelled` que siempre es `false`, así que se comporta igual que antes.
///
/// - `sink(total_files, total_bytes)`: se invoca tras expandir cada origen de primer nivel
///   con los acumulados hasta ese punto (el worker lo usa para emitir `Progress` con throttle).
/// - `cancelled()`: se consulta antes de expandir cada origen; si devuelve `true`, se corta y
///   se devuelve `Ok` con lo acumulado (el worker decide entonces emitir `Cancelled`).
pub(super) fn plan_transfer_with(
    req: &OpRequest,
    sink: &mut dyn FnMut(usize, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<OpPlan, PlanError> {
    let dest = req.dest_dir.clone().ok_or(PlanError::MissingDest)?;
    for src in &req.sources {
        if src.is_dir() && is_inside(&dest, src) {
            return Err(PlanError::DestInsideSource);
        }
    }
    let mut steps = Vec::new();
    let mut total_bytes = 0u64;
    let mut total_files = 0usize;
    for src in &req.sources {
        if cancelled() {
            break;
        }
        let base_to = dest.join(src.file_name().unwrap_or_default());
        // Copiar/mover un origen a SU PROPIO lugar (`dest.join(name) == src`) es un no-op: no
        // generamos pasos `from == to` (que serían ruidosos y, en el caso de carpeta, podrían
        // alimentar un `pre_delete` peligroso aguas arriba). Saltamos ese origen. Comparamos
        // case-insensitive: en Windows `D:\Foto` y `D:\foto` son el MISMO directorio.
        if paths_eq_ci(&base_to, src) {
            continue;
        }
        expand(
            src,
            &base_to,
            &mut steps,
            &mut total_bytes,
            &mut total_files,
            sink,
            cancelled,
        )?;
    }
    Ok(OpPlan {
        steps,
        total_bytes,
        total_files,
        pre_delete: Vec::new(),
    })
}

/// Planifica un borrado recorriendo el árbol en postorden. Además de permitir progreso real,
/// evita que una carpeta grande sea un único paso opaco e incancelable. Los directorios se
/// agregan después de su contenido para que el motor solo tenga que quitar directorios vacíos.
pub fn plan_delete_with(
    req: &OpRequest,
    sink: &mut dyn FnMut(usize, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<OpPlan, PlanError> {
    let mut steps = Vec::new();
    let mut total_bytes = 0u64;
    let mut total_items = 0usize;
    for src in &req.sources {
        expand_delete(
            src,
            &mut steps,
            &mut total_bytes,
            &mut total_items,
            sink,
            cancelled,
        )?;
    }
    Ok(OpPlan {
        steps,
        total_bytes,
        // Para borrado la unidad útil es "elementos": archivos + carpetas.
        total_files: total_items,
        pre_delete: Vec::new(),
    })
}

fn expand_delete(
    path: &Path,
    steps: &mut Vec<OpStep>,
    total_bytes: &mut u64,
    total_items: &mut usize,
    sink: &mut dyn FnMut(usize, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<(), PlanError> {
    if cancelled() {
        return Ok(());
    }
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| PlanError::SourceUnreadable(path.to_path_buf()))?;
    let is_real_dir = meta.is_dir() && !meta.file_type().is_symlink();
    if is_real_dir {
        let entries =
            std::fs::read_dir(path).map_err(|_| PlanError::SourceUnreadable(path.to_path_buf()))?;
        for entry in entries {
            let entry = entry.map_err(|_| PlanError::SourceUnreadable(path.to_path_buf()))?;
            expand_delete(
                &entry.path(),
                steps,
                total_bytes,
                total_items,
                sink,
                cancelled,
            )?;
            if cancelled() {
                return Ok(());
            }
        }
    }
    let bytes = if is_real_dir { 0 } else { meta.len() };
    *total_bytes = total_bytes.saturating_add(bytes);
    *total_items += 1;
    steps.push(OpStep {
        from: Some(path.to_path_buf()),
        to: path.to_path_buf(),
        bytes,
        is_dir: is_real_dir,
    });
    sink(*total_items, *total_bytes);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn expand(
    src: &Path,
    to: &Path,
    steps: &mut Vec<OpStep>,
    total_bytes: &mut u64,
    total_files: &mut usize,
    sink: &mut dyn FnMut(usize, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<(), PlanError> {
    // Cortar limpio si se canceló a mitad del recorrido (el worker emitirá `Cancelled`).
    if cancelled() {
        return Ok(());
    }
    let meta =
        std::fs::metadata(src).map_err(|_| PlanError::SourceUnreadable(src.to_path_buf()))?;
    if meta.is_dir() {
        steps.push(OpStep {
            from: Some(src.to_path_buf()),
            to: to.to_path_buf(),
            bytes: 0,
            is_dir: true,
        });
        let entries =
            std::fs::read_dir(src).map_err(|_| PlanError::SourceUnreadable(src.to_path_buf()))?;
        for entry in entries.flatten() {
            if cancelled() {
                return Ok(());
            }
            let child = entry.path();
            let child_to = to.join(entry.file_name());
            expand(
                &child,
                &child_to,
                steps,
                total_bytes,
                total_files,
                sink,
                cancelled,
            )?;
        }
    } else {
        let bytes = meta.len();
        steps.push(OpStep {
            from: Some(src.to_path_buf()),
            to: to.to_path_buf(),
            bytes,
            is_dir: false,
        });
        *total_bytes += bytes;
        *total_files += 1;
        // Avisar el avance tras CADA archivo: el worker lo aprovecha con throttle para no
        // inundar el canal. El recorrido síncrono pasa un sink vacío (sin costo real).
        sink(*total_files, *total_bytes);
    }
    Ok(())
}

/// ¿`a` y `b` son la MISMA ruta en un FS case-insensitive (Windows trata `D:\Proj` y
/// `D:\proj` como el mismo directorio)? Compara componente a componente, en minúscula,
/// para no dar falsos negativos por capitalización distinta. Mismo estilo que la `key()`
/// del renombrado en lote, pero por componente (no por string completo), para que sea
/// consistente con `path_starts_with_ci`.
pub(super) fn paths_eq_ci(a: &Path, b: &Path) -> bool {
    let ca: Vec<_> = a.components().collect();
    let cb: Vec<_> = b.components().collect();
    ca.len() == cb.len()
        && ca.iter().zip(&cb).all(|(x, y)| {
            x.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&y.as_os_str().to_string_lossy())
        })
}

/// ¿`child` está dentro de (o es igual a) `ancestor`, comparando case-insensitive?
/// Compara componente a componente: evita tanto el falso negativo por capitalización
/// (`D:\foto\backup` bajo `D:\Foto`) como el falso positivo de comparar el string crudo
/// (`D:\fotos` NO está bajo `D:\foto`, aunque sea prefijo textual).
pub(super) fn path_starts_with_ci(child: &Path, ancestor: &Path) -> bool {
    let cc: Vec<_> = child.components().collect();
    let ca: Vec<_> = ancestor.components().collect();
    cc.len() >= ca.len()
        && ca.iter().zip(&cc).all(|(x, y)| {
            x.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&y.as_os_str().to_string_lossy())
        })
}

/// `true` si `inner` está dentro de (o es igual a) `outer`. Case-insensitive (Windows).
fn is_inside(inner: &Path, outer: &Path) -> bool {
    path_starts_with_ci(inner, outer)
}

#[cfg(test)]
mod tests {
    use super::super::{ConflictPolicy, OpKind, OpRequest};
    use super::*;
    use std::fs;

    fn req(kind: OpKind, sources: Vec<PathBuf>, dest: Option<PathBuf>) -> OpRequest {
        OpRequest {
            kind,
            sources,
            dest_dir: dest,
            conflict: ConflictPolicy::Overwrite,
        }
    }

    #[test]
    fn copy_archivo_simple_un_paso_con_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"hola").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let plan = plan(&req(OpKind::Copy, vec![src.clone()], Some(dest.clone()))).unwrap();
        assert_eq!(plan.total_files, 1);
        assert_eq!(plan.total_bytes, 4);
        assert_eq!(plan.steps[0].to, dest.join("a.txt"));
        assert_eq!(plan.steps[0].bytes, 4);
        assert!(!plan.steps[0].is_dir);
    }

    #[test]
    fn duplicate_crea_nombre_unico_y_reserva_colisiones_del_lote() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("informe.txt");
        let second = dir.path().join("resumen.txt");
        fs::write(&first, b"uno").unwrap();
        fs::write(&second, b"dos").unwrap();
        fs::write(dir.path().join("informe - copia.txt"), b"previo").unwrap();

        let plan = plan(&req(
            OpKind::Duplicate,
            vec![first.clone(), first, second],
            None,
        ))
        .unwrap();

        assert_eq!(plan.total_files, 3);
        assert_eq!(plan.total_bytes, 9);
        assert_eq!(plan.steps[0].to, dir.path().join("informe - copia (2).txt"));
        assert_eq!(plan.steps[1].to, dir.path().join("informe - copia (3).txt"));
        assert_eq!(plan.steps[2].to, dir.path().join("resumen - copia.txt"));
        assert!(plan
            .steps
            .iter()
            .all(|step| step.from.as_ref() != Some(&step.to)));
    }

    #[test]
    fn duplicate_carpeta_expande_el_arbol_en_la_misma_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("proyecto");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("nota.txt"), b"hola").unwrap();

        let plan = plan(&req(OpKind::Duplicate, vec![source], None)).unwrap();

        assert_eq!(plan.total_files, 1);
        assert!(plan
            .steps
            .iter()
            .any(|step| step.is_dir && step.to == dir.path().join("proyecto - copia")));
        assert!(plan
            .steps
            .iter()
            .any(|step| step.to == dir.path().join("proyecto - copia").join("nota.txt")));
    }

    #[test]
    fn delete_expande_en_postorden_y_cuenta_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("arbol");
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        let a = root.join("a.txt");
        let b = sub.join("b.txt");
        fs::write(&a, b"12").unwrap();
        fs::write(&b, b"345").unwrap();

        let plan = plan(&req(
            OpKind::Delete { to_trash: false },
            vec![root.clone()],
            None,
        ))
        .unwrap();
        assert_eq!(plan.total_files, 4);
        assert_eq!(plan.total_bytes, 5);
        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps.last().map(|s| &s.to), Some(&root));
        let sub_pos = plan.steps.iter().position(|s| s.to == sub).unwrap();
        let b_pos = plan.steps.iter().position(|s| s.to == b).unwrap();
        assert!(
            b_pos < sub_pos,
            "el contenido debe borrarse antes que su carpeta"
        );
    }

    #[test]
    fn delete_no_sigue_symlink_de_directorio() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("conservar.txt"), b"x").unwrap();
        let link = dir.path().join("link");
        #[cfg(windows)]
        if std::os::windows::fs::symlink_dir(&target, &link).is_err() {
            // Windows puede exigir Developer Mode o privilegio de creación de symlinks.
            return;
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let plan = plan(&req(
            OpKind::Delete { to_trash: false },
            vec![link.clone()],
            None,
        ))
        .unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].to, link);
        assert!(!plan.steps[0].is_dir);
    }

    #[test]
    fn copy_carpeta_recursiva_expande_pasos() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"aa").unwrap();
        fs::create_dir(src.join("sub")).unwrap();
        fs::write(src.join("sub/b.txt"), b"bbb").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let plan = plan(&req(OpKind::Copy, vec![src], Some(dest))).unwrap();
        assert_eq!(plan.total_bytes, 5);
        assert_eq!(plan.total_files, 2);
        assert!(plan.steps.iter().any(|s| s.is_dir));
    }

    #[test]
    fn copy_carpeta_dentro_de_si_misma_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        let dest = src.join("sub");
        fs::create_dir(&dest).unwrap();
        let e = plan(&req(OpKind::Copy, vec![src], Some(dest))).unwrap_err();
        assert_eq!(e, PlanError::DestInsideSource);
    }

    #[test]
    fn copy_carpeta_a_su_propio_lugar_no_genera_pasos() {
        // Copiar/mover una carpeta a SU PROPIO directorio padre (`dest.join(name) == src`) es un
        // no-op para ese origen: no debe producir pasos `from == to` raros. El plan queda vacío.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"datos").unwrap();
        // dest == padre del origen.
        let dest = dir.path().to_path_buf();
        let plan = plan(&req(OpKind::Copy, vec![src], Some(dest))).unwrap();
        assert!(
            plan.steps.is_empty(),
            "copiar a su propio lugar no genera pasos"
        );
        assert_eq!(plan.total_files, 0);
        assert_eq!(plan.total_bytes, 0);
    }

    #[test]
    fn copy_a_su_propio_lugar_no_descarta_los_otros_origenes() {
        // Si un origen es no-op (a su propio lugar) pero hay OTRO origen que vive en otra carpeta,
        // el plan sigue expandiendo el otro. dest = padre del primer origen (lo hace no-op), pero
        // el segundo origen está en una subcarpeta distinta, así que SÍ se copia.
        let dir = tempfile::tempdir().unwrap();
        // mismo/ se "copia" a su propio padre (dir) → no-op.
        let same = dir.path().join("mismo");
        fs::create_dir(&same).unwrap();
        fs::write(same.join("a.txt"), b"aa").unwrap();
        // otro.txt vive en una subcarpeta `fuente/`, no en `dir`, así que copiarlo a `dir` SÍ es real.
        let fuente = dir.path().join("fuente");
        fs::create_dir(&fuente).unwrap();
        let otro = fuente.join("otro.txt");
        fs::write(&otro, b"bbb").unwrap();
        let dest = dir.path().to_path_buf();
        let plan = plan(&req(OpKind::Copy, vec![same, otro], Some(dest.clone()))).unwrap();
        // Solo el archivo "otro.txt" produjo un paso.
        assert_eq!(plan.total_files, 1);
        assert_eq!(plan.total_bytes, 3);
        assert!(plan.steps.iter().any(|s| s.to == dest.join("otro.txt")));
    }

    #[test]
    fn rename_nombre_invalido_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let r = req(
            OpKind::Rename {
                new_name: "a/b.txt".into(),
            },
            vec![src],
            None,
        );
        let e = plan(&r).unwrap_err();
        assert!(matches!(e, PlanError::InvalidName(_)));
    }

    #[test]
    fn batch_rename_ordena_por_dependencia_y_detecta_ciclo() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("foto1.jpg");
        let f2 = dir.path().join("foto2.jpg");
        fs::write(&f1, b"1").unwrap();
        fs::write(&f2, b"2").unwrap();

        // Shift foto1→foto2, foto2→foto3: el paso de foto2 debe ir PRIMERO.
        let r = req(
            OpKind::BatchRename {
                new_names: vec!["foto2.jpg".into(), "foto3.jpg".into()],
            },
            vec![f1.clone(), f2.clone()],
            None,
        );
        let p = plan(&r).unwrap();
        assert_eq!(p.steps.len(), 2);
        assert_eq!(p.steps[0].from, Some(f2.clone()));
        assert_eq!(p.steps[0].to, dir.path().join("foto3.jpg"));
        assert_eq!(p.steps[1].from, Some(f1.clone()));
        assert_eq!(p.steps[1].to, dir.path().join("foto2.jpg"));

        // Swap foto1↔foto2 (ciclo): error de plan.
        let r = req(
            OpKind::BatchRename {
                new_names: vec!["foto2.jpg".into(), "foto1.jpg".into()],
            },
            vec![f1, f2],
            None,
        );
        assert!(matches!(plan(&r), Err(PlanError::InvalidName(_))));
    }

    #[test]
    fn batch_rename_valida_nombres_y_cantidad() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        fs::write(&f, b"x").unwrap();
        let r = req(
            OpKind::BatchRename {
                new_names: vec!["a/b.txt".into()],
            },
            vec![f.clone()],
            None,
        );
        assert!(matches!(plan(&r), Err(PlanError::InvalidName(_))));
        let r = req(OpKind::BatchRename { new_names: vec![] }, vec![f], None);
        assert!(matches!(plan(&r), Err(PlanError::MissingDest)));
    }

    #[test]
    fn create_dir_anidada_arma_la_ruta_completa() {
        // Un nombre con separadores produce UN paso cuyo destino es la jerarquía completa
        // colgando de `dest` (el motor lo crea con create_dir_all).
        let dest = PathBuf::from("C:/work");
        let r = req(
            OpKind::CreateDir {
                name: "a\\b\\c".into(),
            },
            vec![],
            Some(dest.clone()),
        );
        let p = plan(&r).unwrap();
        assert_eq!(p.steps.len(), 1);
        assert!(p.steps[0].is_dir);
        assert_eq!(p.steps[0].from, None);
        assert_eq!(p.steps[0].to, dest.join("a").join("b").join("c"));
    }

    #[test]
    fn create_dir_con_barra_normal_tambien() {
        // El separador `/` se trata igual que `\`.
        let dest = PathBuf::from("C:/work");
        let r = req(
            OpKind::CreateDir { name: "a/b".into() },
            vec![],
            Some(dest.clone()),
        );
        let p = plan(&r).unwrap();
        assert_eq!(p.steps[0].to, dest.join("a").join("b"));
    }

    #[test]
    fn create_dir_rechaza_traversal_y_absoluta() {
        // SEGURIDAD: una carpeta nueva siempre se crea DENTRO del destino; `..` y rutas
        // absolutas se rechazan en el plan (no se puede escapar de la carpeta destino).
        let dest = PathBuf::from("C:/work");
        for bad in ["a\\..\\b", "..\\fuera", "\\abs", "a\\\\b", "a\\b:c"] {
            let r = req(
                OpKind::CreateDir { name: bad.into() },
                vec![],
                Some(dest.clone()),
            );
            assert!(
                matches!(plan(&r), Err(PlanError::InvalidName(_))),
                "{bad} debería rechazarse en el plan"
            );
        }
    }

    #[test]
    fn create_file_anidado_arma_la_ruta_completa() {
        let dest = PathBuf::from("C:/work");
        let r = req(
            OpKind::CreateFile {
                name: "sub\\nota.txt".into(),
            },
            vec![],
            Some(dest.clone()),
        );
        let p = plan(&r).unwrap();
        assert_eq!(p.steps.len(), 1);
        assert!(!p.steps[0].is_dir);
        assert_eq!(p.total_files, 1);
        assert_eq!(p.steps[0].to, dest.join("sub").join("nota.txt"));
    }

    #[test]
    fn copy_sin_dest_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let e = plan(&req(OpKind::Copy, vec![src], None)).unwrap_err();
        assert_eq!(e, PlanError::MissingDest);
    }

    #[test]
    fn paths_eq_ci_ignora_capitalizacion() {
        // Misma ruta con distinta capitalización → iguales (semántica de Windows).
        assert!(paths_eq_ci(Path::new("D:\\Proj"), Path::new("D:\\proj")));
        assert!(paths_eq_ci(
            Path::new("D:\\Foto\\Backup"),
            Path::new("d:\\foto\\backup")
        ));
        // Rutas realmente distintas → NO iguales.
        assert!(!paths_eq_ci(Path::new("D:\\Proj"), Path::new("D:\\Proj2")));
        // Mismo prefijo textual pero distinto número de componentes → NO iguales.
        assert!(!paths_eq_ci(
            Path::new("D:\\foto"),
            Path::new("D:\\foto\\backup")
        ));
    }

    #[test]
    fn path_starts_with_ci_contiene_case_insensitive() {
        // Hijo bajo un ancestro con otra capitalización → contenido.
        assert!(path_starts_with_ci(
            Path::new("D:\\foto\\backup"),
            Path::new("D:\\Foto")
        ));
        // Igual a sí mismo (distinta capitalización) → contenido (es igual o dentro).
        assert!(path_starts_with_ci(
            Path::new("D:\\Foto"),
            Path::new("d:\\foto")
        ));
        // No es un prefijo de componentes → NO contenido (evita el falso positivo del
        // `starts_with` textual: `D:\fotos` NO está bajo `D:\foto`).
        assert!(!path_starts_with_ci(
            Path::new("D:\\fotos"),
            Path::new("D:\\foto")
        ));
        // Hermano, no descendiente.
        assert!(!path_starts_with_ci(
            Path::new("D:\\foto\\a"),
            Path::new("D:\\otra")
        ));
    }
}
