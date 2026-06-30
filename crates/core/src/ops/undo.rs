// Naygo — deshacer operaciones: inversos, validación y re-emisión como OpRequests.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! R2: a partir de una operación TERMINADA (`OpRequest` original + `OpSummary` con
//! los destinos reales por ítem) se construye su INVERSO como `Vec<UndoAction>`.
//! Deshacer = validar que el inverso aún aplica (las rutas existen, nada ocupado) y
//! re-emitirlo como `OpRequest`s normales: corre por el mismo motor de ops (progreso,
//! cancelación, panel). Deshacer una copia/creación manda lo creado a PAPELERA,
//! nunca borra permanente. Delete no es deshacible en v1 (restaurar de la papelera
//! requiere Shell API aparte).

use super::{ConflictPolicy, OpKind, OpOutcome, OpRequest, OpSummary};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Una acción inversa concreta.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UndoAction {
    /// Devolver `now` a `back_to` (inverso de mover Y de renombrar).
    MoveBack { now: PathBuf, back_to: PathBuf },
    /// Mandar a la papelera algo que la op creó (inverso de copiar/crear).
    TrashCreated { path: PathBuf },
}

/// Una operación deshecha-ble del historial.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UndoEntry {
    pub id: u64,
    /// Etiqueta humana de la op original (p. ej. "Mover → D:\\x").
    pub label: String,
    /// Momento de término (epoch secs; la UI lo formatea).
    pub when_epoch_secs: u64,
    pub actions: Vec<UndoAction>,
    /// Ya se deshizo (no se ofrece de nuevo; v1 sin redo).
    pub undone: bool,
}

/// Construye el inverso de una op terminada. `None` = esa op no es deshacible
/// (Delete, o nada terminó `Done`).
pub fn build_undo(req: &OpRequest, summary: &OpSummary) -> Option<Vec<UndoAction>> {
    // Ítems que efectivamente se concretaron, en orden de proceso (con destino REAL y
    // origen explícito).
    let done: Vec<&super::OpItem> = summary
        .items
        .iter()
        .filter(|i| matches!(i.outcome, OpOutcome::Done))
        .collect();
    if done.is_empty() {
        return None;
    }
    let actions: Vec<UndoAction> = match &req.kind {
        OpKind::Rename { .. } => {
            let from = req.sources.first()?;
            let to = &done.first()?.dest;
            vec![UndoAction::MoveBack {
                now: to.clone(),
                back_to: from.clone(),
            }]
        }
        OpKind::Move => {
            // Provenance EXPLÍCITA: cada `OpItem` Done trae su destino REAL (`dest`) y su origen
            // REAL (`src`). Deshacer = devolver cada archivo movido de su `dest` a su `src`. No se
            // re-deriva el origen por nombre+orden (heurística que fallaba con carpetas: una
            // carpeta produce un paso por descendiente, así que hay muchos más ítems que `sources`
            // y el índice se desincronizaba, emitiendo MoveBack a rutas cruzadas).
            //
            // Orden INVERSO al de ejecución: para árboles anidados, devolver primero los
            // descendientes y al final la carpeta contenedora es lo más seguro (la carpeta padre se
            // creó antes que su contenido al mover; al revertir conviene vaciar antes de mover el
            // contenedor de vuelta).
            let mut acts: Vec<UndoAction> = done
                .iter()
                .filter_map(|item| {
                    let back_to = item.src.clone()?;
                    Some(UndoAction::MoveBack {
                        now: item.dest.clone(),
                        back_to,
                    })
                })
                .collect();
            acts.reverse();
            acts
        }
        OpKind::BatchRename { new_names } => {
            // El plan pudo REORDENAR los pasos (dependencias de un shift), así que
            // el summary no sigue el orden de `sources`. Cada destino se reconoce
            // por su ruta esperada `parent(source)/new_name`; el inverso se emite
            // en ORDEN INVERSO de ejecución (deshacer un shift exige desandar al
            // revés: el último renombrado vuelve primero).
            let expected: Vec<(PathBuf, &PathBuf)> = req
                .sources
                .iter()
                .zip(new_names)
                .filter_map(|(src, name)| src.parent().map(|p| (p.join(name), src)))
                .collect();
            let mut acts: Vec<UndoAction> = done
                .iter()
                .filter_map(|item| {
                    let dest = &item.dest;
                    let back = expected.iter().find(|(d, _)| d == dest).map(|(_, s)| *s)?;
                    Some(UndoAction::MoveBack {
                        now: dest.clone(),
                        back_to: back.clone(),
                    })
                })
                .collect();
            acts.reverse();
            acts
        }
        OpKind::Copy | OpKind::CreateDir { .. } | OpKind::CreateFile { .. } => done
            .into_iter()
            .map(|item| UndoAction::TrashCreated {
                path: item.dest.clone(),
            })
            .collect(),
        OpKind::Delete { .. } => return None,
        // El deshacer de comprimir/extraer lo arma el worker de zip (ui-slint), NO build_undo.
        OpKind::Compress { .. } | OpKind::Extract => return None,
    };
    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

/// ¿El inverso aún aplica? Devuelve `Err(motivo)` con la PRIMERA traba encontrada.
/// Es la guarda que hace seguro deshacer fuera de orden: si un paso intermedio dejó
/// las rutas en otro estado, acá se detecta y el deshacer se bloquea con explicación.
/// Es consciente de la SECUENCIA: un destino ocupado se acepta si una acción
/// anterior de la misma lista lo libera (deshacer un shift de batch-rename).
pub fn validate(actions: &[UndoAction]) -> Result<(), String> {
    let key = |p: &PathBuf| p.to_string_lossy().to_lowercase();
    let mut freed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for a in actions {
        match a {
            UndoAction::MoveBack { now, back_to } => {
                if !now.exists() {
                    return Err(format!("ya no existe: {}", now.display()));
                }
                // Ocupado de verdad: existe Y no lo libera un paso previo de esta
                // misma secuencia Y no es el propio archivo (cambio de mayúsculas).
                if back_to.exists() && !freed.contains(&key(back_to)) && key(now) != key(back_to) {
                    return Err(format!("el destino está ocupado: {}", back_to.display()));
                }
                freed.insert(key(now));
                let parent_ok = back_to.parent().map(|p| p.exists()).unwrap_or(false);
                if !parent_ok {
                    return Err(format!(
                        "la carpeta de destino ya no existe: {}",
                        back_to
                            .parent()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    ));
                }
            }
            UndoAction::TrashCreated { path } => {
                if !path.exists() {
                    return Err(format!("ya no existe: {}", path.display()));
                }
            }
        }
    }
    Ok(())
}

/// Re-emite el inverso como `OpRequest`s normales para el motor de ops:
/// - `MoveBack` con el MISMO nombre → Move agrupado por carpeta de destino.
/// - `MoveBack` con nombre distinto (deshacer renames) → UN `BatchRename` con todos
///   los pares EN ORDEN (build_undo ya los emitió en orden seguro; con uno solo,
///   un Rename simple).
/// - `TrashCreated` → un único Delete a papelera.
///
/// Conflictos: `Skip` (deshacer jamás pisa nada; `validate` ya chequeó).
pub fn to_requests(actions: &[UndoAction]) -> Vec<OpRequest> {
    let mut moves: std::collections::BTreeMap<PathBuf, Vec<PathBuf>> =
        std::collections::BTreeMap::new();
    let mut rename_pairs: Vec<(PathBuf, String)> = Vec::new();
    let mut trash: Vec<PathBuf> = Vec::new();
    for a in actions {
        match a {
            UndoAction::MoveBack { now, back_to } => {
                let same_name = now.file_name() == back_to.file_name();
                if same_name {
                    if let Some(parent) = back_to.parent() {
                        moves
                            .entry(parent.to_path_buf())
                            .or_default()
                            .push(now.clone());
                    }
                } else if let Some(name) = back_to.file_name() {
                    // Nombre distinto: si además cambia de carpeta haría falta
                    // mover+renombrar; el rename clásico es en la misma carpeta, que
                    // es el caso real (deshacer Rename). Rename cubre ambos nombres.
                    rename_pairs.push((now.clone(), name.to_string_lossy().into_owned()));
                }
            }
            UndoAction::TrashCreated { path } => trash.push(path.clone()),
        }
    }
    let mut reqs: Vec<OpRequest> = Vec::new();
    for (dest, sources) in moves {
        reqs.push(OpRequest {
            kind: OpKind::Move,
            sources,
            dest_dir: Some(dest),
            conflict: ConflictPolicy::Skip,
        });
    }
    match rename_pairs.len() {
        0 => {}
        1 => {
            let (now, name) = rename_pairs.remove(0);
            reqs.push(OpRequest {
                kind: OpKind::Rename { new_name: name },
                sources: vec![now],
                dest_dir: None,
                conflict: ConflictPolicy::Skip,
            });
        }
        _ => {
            let (sources, new_names) = rename_pairs.into_iter().unzip();
            reqs.push(OpRequest {
                kind: OpKind::BatchRename { new_names },
                sources,
                dest_dir: None,
                conflict: ConflictPolicy::Skip,
            });
        }
    }
    if !trash.is_empty() {
        reqs.push(OpRequest {
            kind: OpKind::Delete { to_trash: true },
            sources: trash,
            dest_dir: None,
            conflict: ConflictPolicy::Skip,
        });
    }
    reqs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    /// Summary SIN origen explícito (`src: None`). Sirve a las ramas que no usan `src`
    /// (Rename, BatchRename, Copy): emparejan por destino/nombre esperado.
    fn summary(items: Vec<(&str, OpOutcome)>) -> OpSummary {
        OpSummary {
            items: items
                .into_iter()
                .map(|(dest, o)| super::super::OpItem {
                    dest: p(dest),
                    outcome: o,
                    src: None,
                })
                .collect(),
            bytes_done: 0,
            elapsed_secs: 0.0,
        }
    }

    /// Summary CON origen explícito por ítem `(dest, outcome, src)`. Es lo que produce el
    /// motor para un Move (cada paso registra su `step.from`); deshacer Move se apoya en
    /// este `src`.
    fn summary_move(items: Vec<(&str, OpOutcome, &str)>) -> OpSummary {
        OpSummary {
            items: items
                .into_iter()
                .map(|(dest, o, src)| super::super::OpItem {
                    dest: p(dest),
                    outcome: o,
                    src: Some(p(src)),
                })
                .collect(),
            bytes_done: 0,
            elapsed_secs: 0.0,
        }
    }

    #[test]
    fn rename_se_invierte_como_moveback() {
        let req = OpRequest {
            kind: OpKind::Rename {
                new_name: "b.txt".into(),
            },
            sources: vec![p("D:/x/a.txt")],
            dest_dir: None,
            conflict: ConflictPolicy::Overwrite,
        };
        let s = summary(vec![("D:/x/b.txt", OpOutcome::Done)]);
        let acts = build_undo(&req, &s).expect("deshacible");
        assert_eq!(
            acts,
            vec![UndoAction::MoveBack {
                now: p("D:/x/b.txt"),
                back_to: p("D:/x/a.txt"),
            }]
        );
    }

    #[test]
    fn move_devuelve_solo_los_done_a_su_origen_explicito() {
        let req = OpRequest {
            kind: OpKind::Move,
            sources: vec![p("D:/a/uno.txt"), p("D:/a/dos.txt"), p("D:/a/tres.txt")],
            dest_dir: Some(p("D:/b")),
            conflict: ConflictPolicy::Skip,
        };
        // dos.txt quedó Skipped (conflicto); uno y tres se movieron. El motor registró el
        // ORIGEN de cada paso, así que deshacer no depende del orden de `sources`.
        let s = summary_move(vec![
            ("D:/b/uno.txt", OpOutcome::Done, "D:/a/uno.txt"),
            ("D:/b/dos.txt", OpOutcome::Skipped, "D:/a/dos.txt"),
            ("D:/b/tres.txt", OpOutcome::Done, "D:/a/tres.txt"),
        ]);
        let acts = build_undo(&req, &s).expect("deshacible");
        // Inverso en orden inverso de ejecución: tres (último Done) vuelve primero, luego uno.
        // El Skipped (dos) no genera ninguna acción.
        assert_eq!(
            acts,
            vec![
                UndoAction::MoveBack {
                    now: p("D:/b/tres.txt"),
                    back_to: p("D:/a/tres.txt"),
                },
                UndoAction::MoveBack {
                    now: p("D:/b/uno.txt"),
                    back_to: p("D:/a/uno.txt"),
                },
            ]
        );
    }

    #[test]
    fn undo_move_de_carpeta_devuelve_cada_archivo_a_su_origen() {
        // Mover una CARPETA `dir/` con 2 archivos dentro. El plan expande la carpeta en un paso por
        // el directorio + un paso por cada descendiente, así que el summary trae MÁS ítems que
        // `sources` (acá: 3 Done para 1 source). La heurística vieja (un Done por source, en orden)
        // se desincronizaba y emitía MoveBack a rutas cruzadas. Con provenance explícita, cada
        // archivo vuelve a SU origen real, sin importar la cantidad de ítems ni el orden.
        let req = OpRequest {
            kind: OpKind::Move,
            sources: vec![p("D:/origen/dir")],
            dest_dir: Some(p("D:/destino")),
            conflict: ConflictPolicy::Skip,
        };
        // Pasos tal como los emite el motor para una carpeta: el dir y luego sus archivos, cada uno
        // con su `src` real.
        let s = summary_move(vec![
            ("D:/destino/dir", OpOutcome::Done, "D:/origen/dir"),
            (
                "D:/destino/dir/a.txt",
                OpOutcome::Done,
                "D:/origen/dir/a.txt",
            ),
            (
                "D:/destino/dir/b.txt",
                OpOutcome::Done,
                "D:/origen/dir/b.txt",
            ),
        ]);
        let acts = build_undo(&req, &s).expect("deshacible");
        // Cada archivo (y la carpeta) vuelve EXACTAMENTE a su origen; en orden inverso de ejecución
        // (los archivos antes que el contenedor).
        assert_eq!(
            acts,
            vec![
                UndoAction::MoveBack {
                    now: p("D:/destino/dir/b.txt"),
                    back_to: p("D:/origen/dir/b.txt"),
                },
                UndoAction::MoveBack {
                    now: p("D:/destino/dir/a.txt"),
                    back_to: p("D:/origen/dir/a.txt"),
                },
                UndoAction::MoveBack {
                    now: p("D:/destino/dir"),
                    back_to: p("D:/origen/dir"),
                },
            ]
        );
        // Y NINGÚN MoveBack apunta a una ruta cruzada (cada now y su back_to comparten file_name).
        for a in &acts {
            if let UndoAction::MoveBack { now, back_to } = a {
                assert_eq!(
                    now.file_name(),
                    back_to.file_name(),
                    "el archivo debe volver a un origen con su mismo nombre, no a uno cruzado"
                );
            }
        }
    }

    #[test]
    fn undo_move_con_archivos_homonimos_de_carpetas_distintas() {
        // Dos sources con el MISMO file_name (`data.txt`) pero distinto padre (`x/` e `y/`), movidos
        // a un único destino donde el motor desambiguó el segundo a "data (2).txt" (conflict-rename).
        // La heurística vieja por file_name los confundía: ambos "data.txt" matcheaban contra
        // cualquier source. Con provenance explícita, cada uno vuelve a SU carpeta correcta.
        let req = OpRequest {
            kind: OpKind::Move,
            sources: vec![p("D:/x/data.txt"), p("D:/y/data.txt")],
            dest_dir: Some(p("D:/dst")),
            conflict: ConflictPolicy::Rename,
        };
        let s = summary_move(vec![
            ("D:/dst/data.txt", OpOutcome::Done, "D:/x/data.txt"),
            // El segundo chocó y el motor lo desambiguó: dest distinto, pero su src es el real.
            ("D:/dst/data (2).txt", OpOutcome::Done, "D:/y/data.txt"),
        ]);
        let acts = build_undo(&req, &s).expect("deshacible");
        // Orden inverso de ejecución: el de `y/` (último) primero. Cada uno a su PADRE correcto.
        assert_eq!(
            acts,
            vec![
                UndoAction::MoveBack {
                    now: p("D:/dst/data (2).txt"),
                    back_to: p("D:/y/data.txt"),
                },
                UndoAction::MoveBack {
                    now: p("D:/dst/data.txt"),
                    back_to: p("D:/x/data.txt"),
                },
            ]
        );
    }

    #[test]
    fn copy_se_invierte_a_papelera_y_delete_no_es_deshacible() {
        let req = OpRequest {
            kind: OpKind::Copy,
            sources: vec![p("D:/a/f.txt")],
            dest_dir: Some(p("D:/b")),
            conflict: ConflictPolicy::Skip,
        };
        let s = summary(vec![("D:/b/f.txt", OpOutcome::Done)]);
        assert_eq!(
            build_undo(&req, &s).expect("deshacible"),
            vec![UndoAction::TrashCreated {
                path: p("D:/b/f.txt")
            }]
        );

        let del = OpRequest {
            kind: OpKind::Delete { to_trash: true },
            sources: vec![p("D:/a/f.txt")],
            dest_dir: None,
            conflict: ConflictPolicy::Skip,
        };
        assert!(build_undo(&del, &summary(vec![("D:/a/f.txt", OpOutcome::Done)])).is_none());
    }

    #[test]
    fn sin_done_no_hay_deshacer() {
        let req = OpRequest {
            kind: OpKind::Copy,
            sources: vec![p("D:/a/f.txt")],
            dest_dir: Some(p("D:/b")),
            conflict: ConflictPolicy::Skip,
        };
        let s = summary(vec![("D:/b/f.txt", OpOutcome::Skipped)]);
        assert!(build_undo(&req, &s).is_none());
    }

    #[test]
    fn validate_detecta_origen_ausente_y_destino_ocupado() {
        let dir = tempfile::tempdir().unwrap();
        let now = dir.path().join("now.txt");
        let back = dir.path().join("back.txt");

        // `now` no existe aún → inválido.
        let acts = vec![UndoAction::MoveBack {
            now: now.clone(),
            back_to: back.clone(),
        }];
        assert!(validate(&acts).is_err());

        // `now` existe y `back` libre → válido.
        std::fs::write(&now, "x").unwrap();
        assert!(validate(&acts).is_ok());

        // `back` ocupado → inválido.
        std::fs::write(&back, "y").unwrap();
        assert!(validate(&acts).is_err());
    }

    #[test]
    fn batch_rename_se_invierte_en_orden_inverso_y_valida_secuencia() {
        // Shift ejecutado: foto2→foto3 primero, foto1→foto2 después (orden del plan).
        let req = OpRequest {
            kind: OpKind::BatchRename {
                new_names: vec!["foto2.jpg".into(), "foto3.jpg".into()],
            },
            sources: vec![p("D:/x/foto1.jpg"), p("D:/x/foto2.jpg")],
            dest_dir: None,
            conflict: ConflictPolicy::Skip,
        };
        let s = summary(vec![
            ("D:/x/foto3.jpg", OpOutcome::Done),
            ("D:/x/foto2.jpg", OpOutcome::Done),
        ]);
        let acts = build_undo(&req, &s).expect("deshacible");
        // Inverso en orden inverso de ejecución: primero foto2→foto1 (libera foto2),
        // después foto3→foto2.
        assert_eq!(
            acts,
            vec![
                UndoAction::MoveBack {
                    now: p("D:/x/foto2.jpg"),
                    back_to: p("D:/x/foto1.jpg"),
                },
                UndoAction::MoveBack {
                    now: p("D:/x/foto3.jpg"),
                    back_to: p("D:/x/foto2.jpg"),
                },
            ]
        );

        // validate consciente de secuencia: con los archivos REALES del estado
        // post-op (foto2 y foto3), el inverso es válido aunque foto2 "esté ocupado"
        // (lo libera la primera acción).
        let dir = tempfile::tempdir().unwrap();
        let f2 = dir.path().join("foto2.jpg");
        let f3 = dir.path().join("foto3.jpg");
        std::fs::write(&f2, b"1").unwrap();
        std::fs::write(&f3, b"2").unwrap();
        let acts = vec![
            UndoAction::MoveBack {
                now: f2.clone(),
                back_to: dir.path().join("foto1.jpg"),
            },
            UndoAction::MoveBack {
                now: f3,
                back_to: f2,
            },
        ];
        assert!(validate(&acts).is_ok());

        // Y los pares se re-emiten como UN BatchRename en el mismo orden.
        let reqs = to_requests(&acts);
        assert_eq!(reqs.len(), 1);
        match &reqs[0].kind {
            OpKind::BatchRename { new_names } => {
                assert_eq!(
                    new_names,
                    &vec!["foto1.jpg".to_string(), "foto2.jpg".into()]
                );
            }
            otro => panic!("se esperaba BatchRename, vino {otro:?}"),
        }
    }

    #[test]
    fn to_requests_agrupa_moves_y_separa_renames_y_papelera() {
        let acts = vec![
            UndoAction::MoveBack {
                now: p("D:/b/uno.txt"),
                back_to: p("D:/a/uno.txt"),
            },
            UndoAction::MoveBack {
                now: p("D:/b/dos.txt"),
                back_to: p("D:/a/dos.txt"),
            },
            UndoAction::MoveBack {
                now: p("D:/x/nuevo.txt"),
                back_to: p("D:/x/viejo.txt"),
            },
            UndoAction::TrashCreated {
                path: p("D:/c/copia.txt"),
            },
        ];
        let reqs = to_requests(&acts);
        assert_eq!(reqs.len(), 3);
        // Move agrupado a D:/a con ambos sources.
        assert!(matches!(reqs[0].kind, OpKind::Move));
        assert_eq!(reqs[0].sources.len(), 2);
        assert_eq!(reqs[0].dest_dir, Some(p("D:/a")));
        // Rename individual.
        assert!(matches!(reqs[1].kind, OpKind::Rename { .. }));
        // Papelera al final.
        assert!(matches!(reqs[2].kind, OpKind::Delete { to_trash: true }));
        assert_eq!(reqs[2].sources, vec![p("D:/c/copia.txt")]);
    }
}
