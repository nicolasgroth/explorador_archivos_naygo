// Naygo — planificación de operaciones: expandir a pasos + validar (recorre FS).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

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
        OpKind::Delete { .. } => plan_delete(req),
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
        // alimentar un `pre_delete` peligroso aguas arriba). Saltamos ese origen.
        if base_to == *src {
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

fn plan_delete(req: &OpRequest) -> Result<OpPlan, PlanError> {
    let mut steps = Vec::new();
    for src in &req.sources {
        if !src.exists() {
            return Err(PlanError::SourceUnreadable(src.clone()));
        }
        let is_dir = src.is_dir();
        steps.push(OpStep {
            from: Some(src.clone()),
            to: src.clone(),
            bytes: 0,
            is_dir,
        });
    }
    let n = steps.iter().filter(|s| !s.is_dir).count();
    Ok(OpPlan {
        steps,
        total_bytes: 0,
        total_files: n,
        pre_delete: Vec::new(),
    })
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

/// `true` si `inner` está dentro de (o es igual a) `outer`.
fn is_inside(inner: &Path, outer: &Path) -> bool {
    inner.starts_with(outer)
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
}
