// Naygo — batch-rename avanzado (R3): plantillas con comodines + preview puro.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Motor del renombrado en lote. TODO es puro y testeable: la UI manda los ítems
//! (ruta + fecha de modificación), el `BatchSpec` (plantilla, buscar/reemplazar,
//! mayúsculas, contador) y los nombres existentes del directorio; `preview` devuelve
//! las filas Antes→Después con su estado (ok / sin cambio / inválido / colisión).
//! La MISMA simulación que detecta colisiones produce el orden de ejecución seguro
//! (shifts tipo foto1→foto2, foto2→foto3 se ordenan solos; los ciclos a↔b se marcan
//! colisión en v1). Aplicar = un `OpKind::BatchRename` (una sola op deshacible).
//!
//! Comodines (alias ES/EN; desconocido queda literal, como `{fecha}` del pegado):
//! `{nombre}`/`{name}`, `{ext}`, `{n}`/`{n:K}` (contador con padding), y de la fecha
//! de modificación del archivo: `{dia}`/`{day}`, `{mes}`/`{month}`, `{año}`/`{anio}`/
//! `{year}`, `{hora}`/`{hour}`, `{min}`, `{seg}`/`{sec}`.

use crate::clipboard::naming::civil_from_epoch;
use crate::ops::is_valid_name;
use std::path::PathBuf;

/// Transformación de mayúsculas aplicada al final del pipeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaseTransform {
    #[default]
    None,
    Lower,
    Upper,
    /// Primera letra de cada palabra en mayúscula, el resto en minúscula.
    Title,
}

/// Especificación del lote (lo que la UI edita en vivo).
#[derive(Clone, Debug)]
pub struct BatchSpec {
    /// Plantilla con comodines. Con `include_ext = false` transforma solo el STEM.
    pub template: String,
    /// `true`: la plantilla produce el nombre COMPLETO (y `{ext}` está disponible).
    /// `false` (default): la extensión original se conserva tal cual.
    pub include_ext: bool,
    /// Buscar (vacío = paso desactivado). Se aplica DESPUÉS de la plantilla.
    pub find: String,
    pub replace: String,
    /// `find`/`replace` como regex (grupos `$1`). Inválida → todas las filas Invalid.
    pub use_regex: bool,
    pub case: CaseTransform,
    /// Contador `{n}`: valor del primer ítem y paso entre ítems.
    pub counter_start: i64,
    pub counter_step: i64,
}

impl Default for BatchSpec {
    fn default() -> Self {
        Self {
            template: "{nombre}".into(),
            include_ext: false,
            find: String::new(),
            replace: String::new(),
            use_regex: false,
            case: CaseTransform::None,
            counter_start: 1,
            counter_step: 1,
        }
    }
}

/// Un ítem del lote: ruta + fecha de modificación (para los comodines de fecha).
#[derive(Clone, Debug)]
pub struct BatchItem {
    pub path: PathBuf,
    pub modified_epoch_secs: Option<u64>,
}

/// Estado de una fila del preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowStatus {
    Ok,
    /// El nombre no cambia (fila atenuada; no cuenta como cambio).
    Unchanged,
    /// Nombre inválido (caracteres prohibidos, vacío) o regex mala — con motivo.
    Invalid(String),
    /// Chocaría con otro resultado del lote, con un archivo existente que no se
    /// libera, o forma un ciclo (swap) — no soportado en v1.
    Collision,
}

/// Una fila Antes→Después del preview.
#[derive(Clone, Debug)]
pub struct PreviewRow {
    pub path: PathBuf,
    pub old_name: String,
    pub new_name: String,
    pub status: RowStatus,
}

/// Calcula el preview completo del lote. `existing_names` = nombres actuales del
/// directorio (incluidos los del propio lote); `utc_offset_secs` = offset local
/// que provee platform (core no conoce la zona horaria).
pub fn preview(
    items: &[BatchItem],
    spec: &BatchSpec,
    existing_names: &[String],
    utc_offset_secs: i64,
) -> Vec<PreviewRow> {
    // Regex compilada una vez; inválida → todas las filas Invalid con el motivo.
    let regex = if spec.use_regex && !spec.find.is_empty() {
        match regex::Regex::new(&spec.find) {
            Ok(r) => Some(r),
            Err(e) => {
                return items
                    .iter()
                    .map(|it| PreviewRow {
                        path: it.path.clone(),
                        old_name: file_name_of(it),
                        new_name: String::new(),
                        status: RowStatus::Invalid(format!("regex: {e}")),
                    })
                    .collect();
            }
        }
    } else {
        None
    };

    let mut rows: Vec<PreviewRow> = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let old_name = file_name_of(it);
            let counter = spec.counter_start + (i as i64) * spec.counter_step;
            let new_name = compute_name(
                &old_name,
                it,
                spec,
                regex.as_ref(),
                counter,
                utc_offset_secs,
            );
            let status = if new_name == old_name {
                RowStatus::Unchanged
            } else if !is_valid_name(&new_name) {
                RowStatus::Invalid(new_name.clone())
            } else {
                RowStatus::Ok // colisiones se resuelven abajo, con el lote completo
            };
            PreviewRow {
                path: it.path.clone(),
                old_name,
                new_name,
                status,
            }
        })
        .collect();

    mark_collisions(&mut rows, existing_names);
    rows
}

/// `true` si el lote se puede aplicar: ninguna fila inválida/colisión y ≥1 cambio.
pub fn can_apply(rows: &[PreviewRow]) -> bool {
    let mut any_change = false;
    for r in rows {
        match r.status {
            RowStatus::Ok => any_change = true,
            RowStatus::Unchanged => {}
            RowStatus::Invalid(_) | RowStatus::Collision => return false,
        }
    }
    any_change
}

/// Orden de ejecución seguro de las filas `Ok` (índices sobre `rows`): cada paso va
/// después de quien libera su destino. Es la MISMA simulación de `mark_collisions`,
/// así que con un preview aplicable nunca falla. La usa el plan del `BatchRename`.
pub fn execution_order(rows: &[PreviewRow]) -> Vec<usize> {
    let changed: Vec<usize> = (0..rows.len())
        .filter(|&i| rows[i].status == RowStatus::Ok)
        .collect();
    simulate(rows, &changed, &occupied_from(rows, &[])).0
}

// ── internos ────────────────────────────────────────────────────────────────

fn file_name_of(it: &BatchItem) -> String {
    it.path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Pipeline de un nombre: plantilla → buscar/reemplazar → mayúsculas → re-adjuntar
/// extensión si la plantilla no la incluye.
fn compute_name(
    old_name: &str,
    it: &BatchItem,
    spec: &BatchSpec,
    regex: Option<&regex::Regex>,
    counter: i64,
    utc_offset_secs: i64,
) -> String {
    // Separar stem/extensión al estilo del rename inline: el punto final manda y
    // los dotfiles (".gitignore") cuentan como sin extensión.
    let split = old_name
        .rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty());
    let (stem, ext) = match split {
        Some((s, e)) => (s, Some(e)),
        None => (old_name, None),
    };

    // `{nombre}` es SIEMPRE el stem; `include_ext` solo decide si la plantilla
    // produce el nombre completo o si la extensión original se re-adjunta al final.
    let mut name = expand_tokens(&spec.template, stem, ext, it, counter, utc_offset_secs);

    if !spec.find.is_empty() {
        name = match regex {
            Some(r) => r.replace_all(&name, spec.replace.as_str()).into_owned(),
            None => name.replace(&spec.find, &spec.replace),
        };
    }

    name = match spec.case {
        CaseTransform::None => name,
        CaseTransform::Lower => name.to_lowercase(),
        CaseTransform::Upper => name.to_uppercase(),
        CaseTransform::Title => title_case(&name),
    };

    if !spec.include_ext {
        if let Some(e) = ext {
            name.push('.');
            name.push_str(e);
        }
    }
    name
}

/// Expande los comodines `{...}` de la plantilla. Desconocidos quedan literales.
fn expand_tokens(
    template: &str,
    base: &str,
    ext: Option<&str>,
    it: &BatchItem,
    counter: i64,
    utc_offset_secs: i64,
) -> String {
    // Fecha de modificación en hora local (epoch + offset, saturando en 0).
    let civil = it
        .modified_epoch_secs
        .map(|s| civil_from_epoch(s.saturating_add_signed(utc_offset_secs)));
    let date_part = |idx: usize| -> String {
        match civil {
            Some((y, mo, d, h, mi, s)) => match idx {
                0 => format!("{d:02}"),
                1 => format!("{mo:02}"),
                2 => format!("{y:04}"),
                3 => format!("{h:02}"),
                4 => format!("{mi:02}"),
                _ => format!("{s:02}"),
            },
            None => String::new(),
        }
    };

    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]); // '{' sin cerrar: literal
            break;
        };
        let token = &after[..close];
        let expanded: Option<String> = match token {
            "nombre" | "name" => Some(base.to_string()),
            "ext" => Some(ext.unwrap_or_default().to_string()),
            "n" => Some(counter.to_string()),
            "dia" | "day" => Some(date_part(0)),
            "mes" | "month" => Some(date_part(1)),
            "año" | "anio" | "year" => Some(date_part(2)),
            "hora" | "hour" => Some(date_part(3)),
            "min" => Some(date_part(4)),
            "seg" | "sec" => Some(date_part(5)),
            _ => token.strip_prefix("n:").and_then(|w| {
                let width: usize = w.parse().ok().filter(|&w| (1..=10).contains(&w))?;
                Some(format!("{counter:0width$}"))
            }),
        };
        match expanded {
            Some(s) => out.push_str(&s),
            None => {
                out.push('{');
                out.push_str(token);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Title Case: primera letra alfabética de cada palabra en mayúscula, el resto en
/// minúscula. Separadores de palabra: espacio, '-', '_' y '.'.
fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_word_start = true;
    for c in s.chars() {
        if c == ' ' || c == '-' || c == '_' || c == '.' {
            at_word_start = true;
            out.push(c);
        } else if at_word_start {
            out.extend(c.to_uppercase());
            at_word_start = false;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// Clave de comparación de nombres en Windows (case-insensitive).
fn key(name: &str) -> String {
    name.to_lowercase()
}

/// Conjunto de nombres ocupados al inicio: los existentes del directorio + los
/// nombres viejos del lote (por si la UI pasó un listado desactualizado).
fn occupied_from(rows: &[PreviewRow], existing: &[String]) -> std::collections::HashSet<String> {
    let mut occ: std::collections::HashSet<String> = existing.iter().map(|n| key(n)).collect();
    for r in rows {
        occ.insert(key(&r.old_name));
    }
    occ
}

/// Simula la ejecución: en cada pasada renombra las filas cuyo destino está libre
/// (o lo libera su propio nombre viejo: cambio solo de mayúsculas). Devuelve el
/// orden logrado y los índices que quedaron atascados (duplicado interno, destino
/// ocupado por algo que no se libera, o ciclo).
fn simulate(
    rows: &[PreviewRow],
    changed: &[usize],
    occupied: &std::collections::HashSet<String>,
) -> (Vec<usize>, Vec<usize>) {
    let mut occ = occupied.clone();
    let mut pending: Vec<usize> = changed.to_vec();
    let mut order = Vec::with_capacity(pending.len());
    loop {
        let mut progressed = false;
        pending.retain(|&i| {
            let new_k = key(&rows[i].new_name);
            let old_k = key(&rows[i].old_name);
            if !occ.contains(&new_k) || new_k == old_k {
                occ.remove(&old_k);
                occ.insert(new_k);
                order.push(i);
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
    (order, pending)
}

/// Marca `Collision` en las filas que la simulación no pudo ejecutar.
fn mark_collisions(rows: &mut [PreviewRow], existing: &[String]) {
    let changed: Vec<usize> = (0..rows.len())
        .filter(|&i| rows[i].status == RowStatus::Ok)
        .collect();
    let occ = occupied_from(rows, existing);
    let (_, stuck) = simulate(rows, &changed, &occ);
    for i in stuck {
        rows[i].status = RowStatus::Collision;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, epoch: Option<u64>) -> BatchItem {
        BatchItem {
            path: PathBuf::from(format!("D:/x/{name}")),
            modified_epoch_secs: epoch,
        }
    }

    fn spec(template: &str) -> BatchSpec {
        BatchSpec {
            template: template.into(),
            ..BatchSpec::default()
        }
    }

    fn names(rows: &[PreviewRow]) -> Vec<&str> {
        rows.iter().map(|r| r.new_name.as_str()).collect()
    }

    #[test]
    fn nombre_y_contador_con_padding() {
        let items = vec![item("a.txt", None), item("b.txt", None)];
        let rows = preview(&items, &spec("{nombre}_{n:3}"), &[], 0);
        assert_eq!(names(&rows), vec!["a_001.txt", "b_002.txt"]);
        assert!(rows.iter().all(|r| r.status == RowStatus::Ok));
    }

    #[test]
    fn contador_con_inicio_y_paso() {
        let items = vec![item("a.txt", None), item("b.txt", None)];
        let mut s = spec("foto{n}");
        s.counter_start = 10;
        s.counter_step = 5;
        assert_eq!(
            names(&preview(&items, &s, &[], 0)),
            vec!["foto10.txt", "foto15.txt"]
        );
    }

    #[test]
    fn comodines_de_fecha_es_y_en_con_offset() {
        // 2021-01-01 13:45:30 UTC; offset -3h (Chile verano) → 10:45:30 local.
        let epoch = 1_609_508_730;
        let items = vec![item("f.jpg", Some(epoch))];
        let rows = preview(
            &items,
            &spec("{año}-{mes}-{dia} {hora}{min}{seg}"),
            &[],
            -3 * 3600,
        );
        assert_eq!(names(&rows), vec!["2021-01-01 104530.jpg"]);
        let rows = preview(
            &items,
            &spec("{year}-{month}-{day} {hour}{min}{sec}"),
            &[],
            -3 * 3600,
        );
        assert_eq!(names(&rows), vec!["2021-01-01 104530.jpg"]);
    }

    #[test]
    fn sin_fecha_los_tokens_expanden_vacio() {
        let rows = preview(&[item("f.txt", None)], &spec("x{dia}{hora}"), &[], 0);
        assert_eq!(names(&rows), vec!["x.txt"]);
    }

    #[test]
    fn token_desconocido_queda_literal() {
        let rows = preview(&[item("f.txt", None)], &spec("{otro}-{nombre}"), &[], 0);
        assert_eq!(names(&rows), vec!["{otro}-f.txt"]);
    }

    #[test]
    fn include_ext_da_control_total_del_nombre() {
        let items = vec![item("informe.pdf", None)];
        let mut s = spec("{nombre}.{ext}.bak");
        s.include_ext = true;
        assert_eq!(names(&preview(&items, &s, &[], 0)), vec!["informe.pdf.bak"]);
    }

    #[test]
    fn buscar_reemplazar_plano_y_orden_del_pipeline() {
        // El find/replace corre DESPUÉS de la plantilla (ve el resultado expandido).
        let items = vec![item("IMG_2021.jpg", None)];
        let mut s = spec("{nombre}");
        s.find = "IMG_".into();
        s.replace = "Foto ".into();
        assert_eq!(names(&preview(&items, &s, &[], 0)), vec!["Foto 2021.jpg"]);
    }

    #[test]
    fn regex_con_grupos() {
        let items = vec![item("2021-01-05 viaje.jpg", None)];
        let mut s = spec("{nombre}");
        s.use_regex = true;
        s.find = r"^(\d{4})-(\d{2})-(\d{2}) ".into();
        s.replace = "$3-$2-$1 ".into();
        assert_eq!(
            names(&preview(&items, &s, &[], 0)),
            vec!["05-01-2021 viaje.jpg"]
        );
    }

    #[test]
    fn regex_invalida_marca_todas_las_filas() {
        let items = vec![item("a.txt", None), item("b.txt", None)];
        let mut s = spec("{nombre}");
        s.use_regex = true;
        s.find = "(".into();
        let rows = preview(&items, &s, &[], 0);
        assert!(rows
            .iter()
            .all(|r| matches!(r.status, RowStatus::Invalid(_))));
        assert!(!can_apply(&rows));
    }

    #[test]
    fn transformaciones_de_mayusculas() {
        let items = vec![item("MI archivo-de_prueba.TXT", None)];
        let mut s = spec("{nombre}");
        s.case = CaseTransform::Lower;
        assert_eq!(
            names(&preview(&items, &s, &[], 0)),
            vec!["mi archivo-de_prueba.TXT"]
        );
        s.case = CaseTransform::Title;
        assert_eq!(
            names(&preview(&items, &s, &[], 0)),
            vec!["Mi Archivo-De_Prueba.TXT"]
        );
    }

    #[test]
    fn sin_cambio_es_unchanged_y_no_habilita_aplicar() {
        let rows = preview(
            &[item("a.txt", None)],
            &spec("{nombre}"),
            &["a.txt".into()],
            0,
        );
        assert_eq!(rows[0].status, RowStatus::Unchanged);
        assert!(!can_apply(&rows));
    }

    #[test]
    fn nombre_invalido_se_marca() {
        let mut s = spec("{nombre}");
        s.find = "a".into();
        s.replace = "a/b".into();
        let rows = preview(&[item("a.txt", None)], &s, &[], 0);
        assert!(matches!(rows[0].status, RowStatus::Invalid(_)));
    }

    #[test]
    fn duplicado_interno_es_colision() {
        let items = vec![item("a.txt", None), item("b.txt", None)];
        let rows = preview(&items, &spec("igual"), &[], 0);
        // El primero alcanza a tomar el nombre; el segundo queda en colisión.
        assert_eq!(rows[0].status, RowStatus::Ok);
        assert_eq!(rows[1].status, RowStatus::Collision);
        assert!(!can_apply(&rows));
    }

    #[test]
    fn choque_con_existente_que_no_se_libera_es_colision() {
        let existing = vec!["a.txt".into(), "ocupado.txt".into()];
        let rows = preview(&[item("a.txt", None)], &spec("ocupado"), &existing, 0);
        assert_eq!(rows[0].status, RowStatus::Collision);
    }

    #[test]
    fn shift_se_permite_y_ordena_y_swap_se_bloquea() {
        // Shift: foto1→foto2, foto2→foto3 (foto2 se libera) → válido, orden 2,3 antes.
        let items = vec![item("foto1.jpg", None), item("foto2.jpg", None)];
        let existing: Vec<String> = vec!["foto1.jpg".into(), "foto2.jpg".into()];
        let mut s = spec("foto{n}");
        s.counter_start = 2;
        let rows = preview(&items, &s, &existing, 0);
        assert!(rows.iter().all(|r| r.status == RowStatus::Ok));
        // El orden de ejecución libera foto2 primero (índice 1 antes que 0).
        assert_eq!(execution_order(&rows), vec![1, 0]);

        // Swap genuino a↔b (ciclo) → colisión en v1; filas armadas a mano.
        let rows = vec![
            PreviewRow {
                path: PathBuf::from("D:/x/a.txt"),
                old_name: "a.txt".into(),
                new_name: "b.txt".into(),
                status: RowStatus::Ok,
            },
            PreviewRow {
                path: PathBuf::from("D:/x/b.txt"),
                old_name: "b.txt".into(),
                new_name: "a.txt".into(),
                status: RowStatus::Ok,
            },
        ];
        let mut rows = rows;
        mark_collisions(&mut rows, &["a.txt".into(), "b.txt".into()]);
        assert!(rows.iter().all(|r| r.status == RowStatus::Collision));
    }

    #[test]
    fn cambio_solo_de_mayusculas_se_permite() {
        let existing = vec!["informe.txt".into()];
        let mut s = spec("{nombre}");
        s.case = CaseTransform::Upper;
        let rows = preview(&[item("informe.txt", None)], &s, &existing, 0);
        assert_eq!(rows[0].new_name, "INFORME.txt");
        assert_eq!(rows[0].status, RowStatus::Ok);
    }

    #[test]
    fn can_apply_exige_al_menos_un_cambio_y_cero_problemas() {
        assert!(!can_apply(&[]));
        let rows = preview(
            &[item("a.txt", None), item("b.txt", None)],
            &spec("{nombre}x"),
            &[],
            0,
        );
        assert!(can_apply(&rows));
    }
}
