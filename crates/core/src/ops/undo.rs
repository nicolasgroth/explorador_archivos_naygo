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
    // Destinos que efectivamente se concretaron, en orden de proceso.
    let done: Vec<&PathBuf> = summary
        .items
        .iter()
        .filter(|(_, o)| matches!(o, OpOutcome::Done))
        .map(|(p, _)| p)
        .collect();
    if done.is_empty() {
        return None;
    }
    let actions: Vec<UndoAction> = match &req.kind {
        OpKind::Rename { .. } => {
            let from = req.sources.first()?;
            let to = done.first()?;
            vec![UndoAction::MoveBack {
                now: (*to).clone(),
                back_to: from.clone(),
            }]
        }
        OpKind::Move => {
            // Emparejar por NOMBRE DE ORIGEN: el summary trae los destinos reales
            // (robusto ante conflict-rename, donde el destino cambió de nombre los
            // emparejamos por posición dentro de los Done, que respetan el orden del
            // plan = orden de sources tras el filtrado de Skipped).
            let done_in_order = done;
            let mut acts = Vec::new();
            let mut di = 0usize;
            for src in &req.sources {
                let Some(dest) = done_in_order.get(di) else {
                    break;
                };
                // El paso de `src` pudo haber sido Skipped/Failed: solo consumimos un
                // Done si su nombre coincide con el de src O si hubo conflict-rename
                // (nombre distinto pero mismo orden). Heurística por orden con chequeo
                // de nombre para no desfasar ante saltos.
                let name_matches = dest.file_name() == src.file_name();
                let skipped_or_failed = summary.items.iter().any(|(p, o)| {
                    !matches!(o, OpOutcome::Done) && p.file_name() == src.file_name()
                });
                if !name_matches && skipped_or_failed {
                    continue; // este source no se movió; no consumir el Done
                }
                acts.push(UndoAction::MoveBack {
                    now: (*dest).clone(),
                    back_to: src.clone(),
                });
                di += 1;
            }
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
                .filter_map(|dest| {
                    let back = expected.iter().find(|(d, _)| d == *dest).map(|(_, s)| *s)?;
                    Some(UndoAction::MoveBack {
                        now: (*dest).clone(),
                        back_to: back.clone(),
                    })
                })
                .collect();
            acts.reverse();
            acts
        }
        OpKind::Copy | OpKind::CreateDir { .. } | OpKind::CreateFile { .. } => done
            .into_iter()
            .map(|p| UndoAction::TrashCreated { path: p.clone() })
            .collect(),
        OpKind::Delete { .. } => return None,
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

    fn summary(items: Vec<(&str, OpOutcome)>) -> OpSummary {
        OpSummary {
            items: items.into_iter().map(|(s, o)| (p(s), o)).collect(),
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
    fn move_empareja_sources_con_done_y_excluye_skipped() {
        let req = OpRequest {
            kind: OpKind::Move,
            sources: vec![p("D:/a/uno.txt"), p("D:/a/dos.txt"), p("D:/a/tres.txt")],
            dest_dir: Some(p("D:/b")),
            conflict: ConflictPolicy::Skip,
        };
        // dos.txt quedó Skipped (conflicto); uno y tres se movieron.
        let s = summary(vec![
            ("D:/b/uno.txt", OpOutcome::Done),
            ("D:/b/dos.txt", OpOutcome::Skipped),
            ("D:/b/tres.txt", OpOutcome::Done),
        ]);
        let acts = build_undo(&req, &s).expect("deshacible");
        assert_eq!(
            acts,
            vec![
                UndoAction::MoveBack {
                    now: p("D:/b/uno.txt"),
                    back_to: p("D:/a/uno.txt"),
                },
                UndoAction::MoveBack {
                    now: p("D:/b/tres.txt"),
                    back_to: p("D:/a/tres.txt"),
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
