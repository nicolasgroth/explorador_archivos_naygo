// Naygo — operaciones de archivo: modelo de tipos (puro, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Tipos que describen una operación de archivo (copiar/mover/eliminar/renombrar/
//! crear), su plan (pasos + bytes), los mensajes del motor (progreso/conflicto/fin)
//! y el resumen. La planificación (`plan`) y la ejecución (`engine`) viven en sus
//! propios submódulos. Todo el modelo es serializable (útil para el journal de ops-B).

pub mod actions;
pub mod engine;
pub mod journal;
pub mod names;
pub mod plan;
pub mod plan_async;
pub mod undo;

pub use actions::{
    batch_rename, create, delete, parse_new_folders, rename, transfer, FolderSpec, NewFolderError,
};
pub use engine::{run_plan, spawn};
pub use journal::{
    journal_path, remove, resume_plan, scan, FileFingerprint, JournalWriter, OpJournal, ResumePlan,
};
pub use names::{dedup_name, is_valid_name};
pub use plan::{plan, PlanError};
pub use plan_async::{spawn_plan, PlanMsg};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Qué operación se pide.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpKind {
    Copy,
    Move,
    Delete {
        to_trash: bool,
    },
    Rename {
        new_name: String,
    },
    /// Renombrar en lote (R3): `new_names[i]` es el nombre nuevo de `sources[i]`.
    /// Cada paso renombra dentro de su propia carpeta. UNA sola op → UNA entrada
    /// en el journal/historial (deshacible como un paso).
    BatchRename {
        new_names: Vec<String>,
    },
    CreateDir {
        name: String,
    },
    CreateFile {
        name: String,
    },
}

/// Qué hacer ante un nombre que ya existe en el destino.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictPolicy {
    /// Preguntar a la UI (emite `OpMsg::Conflict` y espera la decisión).
    Ask,
    Overwrite,
    Skip,
    Rename,
}

/// Solicitud de operación, armada por la UI desde la selección.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpRequest {
    pub kind: OpKind,
    pub sources: Vec<PathBuf>,
    /// Carpeta destino (Copy/Move). `None` para Delete/Rename/Create.
    pub dest_dir: Option<PathBuf>,
    pub conflict: ConflictPolicy,
}

/// Un paso concreto del plan: copiar/mover un archivo, o crear una carpeta.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpStep {
    /// Origen (None para crear carpeta vacía en el destino).
    pub from: Option<PathBuf>,
    pub to: PathBuf,
    pub bytes: u64,
    pub is_dir: bool,
}

/// Plan completo de una operación: pasos + totales.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpPlan {
    pub steps: Vec<OpStep>,
    pub total_bytes: u64,
    pub total_files: usize,
    /// Carpetas destino a BORRAR (`remove_dir_all`) antes de copiar, por una decisión
    /// "Reemplazar" en un conflicto de carpeta. El motor las procesa al inicio de `run_plan`
    /// (cancelable, error tipado). Vacío en el caso normal. `#[serde(default)]` para que los
    /// journals viejos (sin este campo) sigan deserializando.
    #[serde(default)]
    pub pre_delete: Vec<PathBuf>,
}

/// Progreso emitido por el motor mientras trabaja.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpProgress {
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: usize,
    pub files_total: usize,
    pub current: PathBuf,
}

/// Petición de decisión de conflicto que el motor manda a la UI.
///
/// Además de las dos rutas (el archivo que YA existe en el destino y el que llega), trae los
/// METADATOS de ambos lados (tamaño, fecha de modificación, si es carpeta) para que la UI los
/// muestre LADO A LADO ("Existente" | "Nuevo") sin volver a tocar el disco. Los metadatos se leen
/// con `fs::metadata` al armar el prompt (`from_paths`): si una ruta no se puede leer, su tamaño
/// queda en 0 y su fecha en `None` (la UI muestra "—"). Los campos de metadatos llevan
/// `#[serde(default)]` para que un journal/serialización vieja (solo con las rutas) siga
/// deserializando.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConflictPrompt {
    pub existing: PathBuf,
    pub incoming: PathBuf,
    /// Tamaño del archivo EXISTENTE en bytes (0 si no se pudo leer o es carpeta).
    #[serde(default)]
    pub existing_size: u64,
    /// Fecha de modificación del EXISTENTE en segundos epoch UTC (`None` si no se pudo leer).
    #[serde(default)]
    pub existing_modified: Option<u64>,
    /// `true` si el EXISTENTE es una carpeta.
    #[serde(default)]
    pub existing_is_dir: bool,
    /// Tamaño del archivo NUEVO (entrante) en bytes (0 si no se pudo leer o es carpeta).
    #[serde(default)]
    pub incoming_size: u64,
    /// Fecha de modificación del NUEVO en segundos epoch UTC (`None` si no se pudo leer).
    #[serde(default)]
    pub incoming_modified: Option<u64>,
    /// `true` si el NUEVO es una carpeta.
    #[serde(default)]
    pub incoming_is_dir: bool,
}

impl ConflictPrompt {
    /// Arma un prompt leyendo los metadatos de ambas rutas con `fs::metadata`. Tolerante a fallos:
    /// si una ruta no se puede leer (permiso, desaparecida, disco caído), su tamaño queda en 0 y su
    /// fecha en `None` en vez de fallar; el conflicto se sigue pudiendo resolver con las rutas.
    pub fn from_paths(existing: PathBuf, incoming: PathBuf) -> ConflictPrompt {
        let (existing_size, existing_modified, existing_is_dir) = read_meta(&existing);
        let (incoming_size, incoming_modified, incoming_is_dir) = read_meta(&incoming);
        ConflictPrompt {
            existing,
            incoming,
            existing_size,
            existing_modified,
            existing_is_dir,
            incoming_size,
            incoming_modified,
            incoming_is_dir,
        }
    }
}

/// Lee (tamaño, modificado epoch UTC, es_carpeta) de una ruta. Tolerante a fallos: ante cualquier
/// error de I/O devuelve `(0, None, false)`.
fn read_meta(p: &Path) -> (u64, Option<u64>, bool) {
    match std::fs::metadata(p) {
        Ok(m) => {
            let modified = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            // Un directorio reporta `len()` del nodo, no del contenido: lo dejamos en 0 para no
            // mostrar un tamaño engañoso (la UI distingue carpeta de archivo por `is_dir`).
            let size = if m.is_dir() { 0 } else { m.len() };
            (size, modified, m.is_dir())
        }
        Err(_) => (0, None, false),
    }
}

/// `true` si dos archivos se consideran IDÉNTICOS por una comparación BARATA de metadatos: mismo
/// tamaño Y misma fecha de modificación (con tolerancia de 2 segundos para los sistemas de archivos
/// tipo FAT, que guardan la hora con resolución de 2s). NO compara contenido byte a byte ni hash:
/// es la heurística que usa "saltar idénticos" para no recopiar lo que ya está igual en el destino.
/// Si alguna fecha no se puede leer, NO se consideran idénticos (mejor preguntar que asumir).
pub fn are_identical(a: &Path, b: &Path) -> bool {
    let (size_a, mod_a, dir_a) = read_meta(a);
    let (size_b, mod_b, dir_b) = read_meta(b);
    // Carpetas: nunca "idénticas" por este criterio (no tienen tamaño/contenido comparable así).
    if dir_a || dir_b {
        return false;
    }
    if size_a != size_b {
        return false;
    }
    match (mod_a, mod_b) {
        (Some(ta), Some(tb)) => ta.abs_diff(tb) <= 2,
        // Sin fecha de un lado: no asumimos identidad.
        _ => false,
    }
}

/// Decisión que la UI devuelve al motor ante un conflicto.
///
/// No es `Copy` porque `ConflictAction::RenameTo` lleva un `String` (el nombre elegido por el
/// usuario). Se clona donde haga falta.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictDecision {
    pub action: ConflictAction,
    /// Aplicar esta decisión a todos los conflictos siguientes de la op.
    pub apply_all: bool,
}

/// Acción concreta de un conflicto resuelto.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictAction {
    Overwrite,
    Skip,
    /// Renombrar con sufijo automático " (N)" (`dedup_name`). Es la red de seguridad y el
    /// comportamiento "rápido": no pide nada al usuario.
    Rename,
    /// Renombrar con un nombre ELEGIDO por el usuario (el que escribió en el modal de nombre).
    /// El destino pasa a ser `step.to.with_file_name(nombre)`. Si ese nombre TAMBIÉN choca, el
    /// motor aplica `dedup_name` sobre él como salvaguarda (nunca falla ni pisa). No tiene
    /// sentido con "aplicar a todos" (cada archivo necesita su propio nombre); la UI lo
    /// deshabilita en ese caso.
    RenameTo(String),
    /// Renombrar el archivo EXISTENTE (el del destino) con un sufijo automático " (N)" libre, y
    /// dejar entrar el NUEVO con su nombre original. Es la inversa de `Rename`: en vez de
    /// desambiguar el entrante, desambigua el que ya estaba. Útil cuando se quiere conservar AMBOS
    /// pero que el nuevo se quede con el nombre "bueno". Si el rename del existente falla
    /// (permiso/archivo en uso), el motor reporta el paso como fallido y NO pisa el existente.
    RenameExisting,
    /// POLÍTICA "saltar idénticos": al elegirla (típicamente con "aplicar a todos"), el motor
    /// salta automáticamente los conflictos donde el existente y el entrante son IDÉNTICOS por
    /// metadatos (`are_identical`: mismo tamaño y fecha, tolerancia 2s) y, para los que DIFIEREN,
    /// vuelve a preguntar a la UI. Como acción por-ítem suelta (sin aplicar a todos) se comporta
    /// como `Skip` si son idénticos o `Overwrite` si difieren — pero su uso natural es como
    /// política, así que la UI la ofrece con "aplicar a todos".
    SkipIdentical,
}

/// Qué hacer cuando una CARPETA de origen ya existe (como carpeta) en el destino. A
/// diferencia de `ConflictAction` (que es por ARCHIVO), esto se decide UNA vez por carpeta,
/// antes de copiar, sobre el árbol completo de ese origen. `Cancel` no es una variante: se
/// modela cancelando la op (igual que cancelar un conflicto por archivo).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FolderDecision {
    /// Copiar dentro de la carpeta existente; los archivos que choquen disparan el conflicto
    /// por archivo (Ask). Es el comportamiento por defecto: no toca el plan.
    Merge,
    /// Borrar la carpeta destino entera (`remove_dir_all`) y copiar limpio. DESTRUCTIVO.
    Replace,
    /// No copiar esa carpeta: se excluyen del plan todos los pasos bajo su destino.
    Skip,
}

/// Un conflicto de carpeta detectado en el pre-check: una carpeta de origen cuyo destino
/// `dest.join(nombre)` YA existe como carpeta. `name` es para mostrar en el diálogo;
/// `dest_root` es la ruta destino exacta a borrar (Replace) o filtrar (Skip).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderConflict {
    /// Nombre de la carpeta (para el texto del diálogo).
    pub name: String,
    /// Origen (la carpeta que se está copiando/moviendo).
    pub source: PathBuf,
    /// Destino exacto que ya existe: `dest_dir.join(name)`.
    pub dest_root: PathBuf,
}

/// Detecta los conflictos de CARPETA de un `OpRequest`: orígenes que son directorios cuyo
/// destino `dest_dir.join(nombre)` ya existe como directorio. Un origen archivo cuyo destino
/// existe NO es un conflicto de carpeta (lo maneja el conflicto por archivo). Solo aplica a
/// Copy/Move; para otros tipos devuelve vacío. Es puro salvo por consultar el FS (igual que
/// `first_collision` en la UI).
pub fn folder_conflicts(req: &OpRequest) -> Vec<FolderConflict> {
    let Some(dest) = req.dest_dir.as_ref() else {
        return Vec::new();
    };
    if !matches!(req.kind, OpKind::Copy | OpKind::Move) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for src in &req.sources {
        if !src.is_dir() {
            continue;
        }
        let Some(name) = src.file_name() else {
            continue;
        };
        let dest_root = dest.join(name);
        // GUARDA CRÍTICA: copiar/mover una carpeta a SU PROPIO directorio padre hace que
        // `dest_root == src` (la carpeta de origen MISMA). Eso NO es un conflicto: reportarlo
        // permitiría que el usuario eligiera "Reemplazar" y el motor borrara el origen entero
        // (pérdida total de datos). Lo descartamos antes de siquiera tocar el FS. La comparación
        // es case-insensitive: en Windows `D:\Foto` y `D:\foto` son el MISMO directorio.
        if plan::paths_eq_ci(&dest_root, src) {
            continue;
        }
        if dest_root.is_dir() {
            out.push(FolderConflict {
                name: name.to_string_lossy().into_owned(),
                source: src.clone(),
                dest_root,
            });
        }
    }
    out
}

/// Aplica una `FolderDecision` sobre el conflicto de carpeta cuyo destino es `dest_root`,
/// MUTANDO el plan en su lugar:
/// - `Merge`: no toca nada (el plan ya copia dentro de la existente).
/// - `Skip`: excluye todos los pasos cuyo destino cae bajo `dest_root` (no se copia ese
///   subárbol) y recalcula los totales del plan.
/// - `Replace`: agrega `dest_root` a `plan.pre_delete` (el motor lo borra con `remove_dir_all`
///   ANTES de copiar, de forma cancelable y con error tipado). No quita pasos: tras borrar, la
///   copia repuebla el destino con SOLO el contenido del origen.
///
/// `dest_root` es la ruta destino exacta (`dest_dir.join(nombre)`): solo se filtra/borra ESE
/// destino, nunca el origen ni otra carpeta.
pub fn apply_folder_decision(plan: &mut OpPlan, dest_root: &Path, decision: FolderDecision) {
    match decision {
        FolderDecision::Merge => {}
        FolderDecision::Skip => {
            plan.steps.retain(|s| !s.to.starts_with(dest_root));
            recompute_totals(plan);
        }
        FolderDecision::Replace => {
            if !plan.pre_delete.iter().any(|p| p.as_path() == dest_root) {
                plan.pre_delete.push(dest_root.to_path_buf());
            }
        }
    }
}

/// Recalcula `total_bytes`/`total_files` de un plan a partir de sus pasos (tras filtrar).
fn recompute_totals(plan: &mut OpPlan) {
    plan.total_bytes = plan.steps.iter().map(|s| s.bytes).sum();
    plan.total_files = plan.steps.iter().filter(|s| !s.is_dir).count();
}

/// Resultado por archivo, para el resumen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OpOutcome {
    Done,
    Skipped,
    Failed(String),
}

/// Un ítem procesado por la operación: a dónde fue (`dest`), con qué resultado
/// (`outcome`), y de dónde vino (`src`, `None` para "crear carpeta/archivo vacío"). El
/// `src` permite deshacer un Move emparejando por provenance EXPLÍCITA (no re-derivando
/// el origen por nombre, heurística que falla con carpetas: una carpeta produce un paso
/// por descendiente, así que hay muchos más ítems que `sources`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpItem {
    /// Ruta destino REAL del paso (puede diferir de `step.to` por conflict-rename).
    pub dest: PathBuf,
    pub outcome: OpOutcome,
    /// Origen del paso que se ejecutó (`step.from`). `None` para crear vacío.
    pub src: Option<PathBuf>,
}

/// Resumen de una operación terminada (o cancelada).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpSummary {
    /// Un `OpItem` (destino + resultado + origen) por archivo procesado.
    pub items: Vec<OpItem>,
    pub bytes_done: u64,
    pub elapsed_secs: f64,
}

impl OpSummary {
    pub fn count_done(&self) -> usize {
        self.items
            .iter()
            .filter(|i| matches!(i.outcome, OpOutcome::Done))
            .count()
    }
    pub fn count_skipped(&self) -> usize {
        self.items
            .iter()
            .filter(|i| matches!(i.outcome, OpOutcome::Skipped))
            .count()
    }
    pub fn count_failed(&self) -> usize {
        self.items
            .iter()
            .filter(|i| matches!(i.outcome, OpOutcome::Failed(_)))
            .count()
    }
}

/// Mensajes que el motor emite hacia la UI por el canal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OpMsg {
    Progress(OpProgress),
    Conflict(ConflictPrompt),
    Done(OpSummary),
    Cancelled(OpSummary),
    /// Error fatal que impide siquiera empezar (p. ej. plan inválido).
    Failed(String),
}

#[cfg(test)]
mod folder_conflict_tests {
    use super::plan::plan;
    use super::*;
    use std::fs;

    /// Árbol de origen `carpeta/` con un archivo, y un destino que YA tiene una `carpeta/`
    /// (poblada con un archivo extra que NO está en el origen). Devuelve (tempdir, req, dest_root).
    fn escenario_conflicto_carpeta() -> (tempfile::TempDir, OpRequest, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        // Origen: carpeta/ con a.txt.
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"origen").unwrap();
        // Destino: dst/ que ya contiene carpeta/ con un archivo EXTRA (solo del destino).
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let dest_root = dest.join("carpeta");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("extra.txt"), b"solo-en-destino").unwrap();
        let req = transfer(false, vec![src], dest);
        (dir, req, dest_root)
    }

    #[test]
    fn folder_conflicts_detecta_carpeta_existente() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let conflicts = folder_conflicts(&req);
        assert_eq!(conflicts.len(), 1, "una carpeta en conflicto");
        assert_eq!(conflicts[0].name, "carpeta");
        assert_eq!(conflicts[0].dest_root, dest_root);
    }

    #[test]
    fn folder_conflicts_ignora_archivo_que_choca() {
        // Un ARCHIVO de origen cuyo destino existe NO es conflicto de carpeta (lo maneja el
        // conflicto por archivo). folder_conflicts debe devolver vacío.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"nuevo").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), b"viejo").unwrap();
        let req = transfer(false, vec![src], dest);
        assert!(folder_conflicts(&req).is_empty());
    }

    #[test]
    fn folder_conflicts_ignora_carpeta_nueva() {
        // El destino no tiene la carpeta → no hay conflicto.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = transfer(false, vec![src], dest);
        assert!(folder_conflicts(&req).is_empty());
    }

    #[test]
    fn folder_conflicts_ignora_carpeta_a_su_propio_lugar() {
        // BUG CRÍTICO: copiar/mover una carpeta a SU PROPIO directorio padre hace que
        // `dest.join(nombre) == src` (el origen MISMO). Eso NO es un conflicto: si se reportara,
        // el usuario podría elegir Reemplazar y el motor borraría el origen. folder_conflicts
        // DEBE descartar ese caso.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"origen").unwrap();
        // dest == padre del origen → dest.join("carpeta") == src.
        let dest = dir.path().to_path_buf();
        let req = transfer(false, vec![src], dest);
        assert!(
            folder_conflicts(&req).is_empty(),
            "copiar una carpeta a su propio lugar no es un conflicto"
        );
    }

    #[test]
    fn folder_conflicts_ignora_su_propio_lugar_aunque_cambie_la_capitalizacion() {
        // BUG DE PÉRDIDA DE DATOS (case-sensitivity): copiar una carpeta a su propio lugar pero
        // con el directorio padre escrito con OTRA capitalización (`sub` vs `SUB`) producía
        // `dest_root != src` con `==` crudo, así que se reportaba como conflicto. El usuario
        // podía elegir Reemplazar y el motor borraba el ORIGEN. En Windows `sub` y `SUB` son el
        // MISMO directorio: la guarda case-insensitive DEBE descartar este caso.
        let dir = tempfile::tempdir().unwrap();
        // El directorio real se llama "sub"; dentro vive la carpeta a copiar.
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let src = sub.join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"origen").unwrap();
        // El destino apunta al MISMO padre pero con otra capitalización: dest.join("carpeta")
        // será ".../SUB/carpeta", que en un FS case-insensitive es la carpeta de origen MISMA.
        let dest = dir.path().join("SUB");
        let req = transfer(false, vec![src], dest);
        assert!(
            folder_conflicts(&req).is_empty(),
            "con distinta capitalización del padre sigue siendo el propio lugar: no es conflicto"
        );
    }

    #[test]
    fn merge_no_toca_el_plan() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let mut p = plan(&req).unwrap();
        let antes = p.clone();
        apply_folder_decision(&mut p, &dest_root, FolderDecision::Merge);
        assert_eq!(p, antes, "Merge deja el plan idéntico");
        assert!(p.pre_delete.is_empty());
    }

    #[test]
    fn skip_excluye_los_pasos_bajo_la_carpeta() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let mut p = plan(&req).unwrap();
        // Antes: hay pasos cuyo destino cae bajo dest_root (la carpeta y su a.txt).
        assert!(p.steps.iter().any(|s| s.to.starts_with(&dest_root)));
        let bytes_archivo = p.total_bytes; // "origen" = 6 bytes
        assert!(bytes_archivo > 0);

        apply_folder_decision(&mut p, &dest_root, FolderDecision::Skip);

        // Después: NINGÚN paso cae bajo dest_root y los totales se recalcularon a cero.
        assert!(
            !p.steps.iter().any(|s| s.to.starts_with(&dest_root)),
            "Skip excluye todo el subárbol de la carpeta"
        );
        assert_eq!(p.total_bytes, 0);
        assert_eq!(p.total_files, 0);
        assert!(p.pre_delete.is_empty(), "Skip no borra nada");
    }

    #[test]
    fn skip_no_toca_otros_origenes() {
        // Dos carpetas a copiar; solo una choca. Skip de la que choca NO debe quitar los pasos
        // de la otra.
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("x.txt"), b"xx").unwrap();
        fs::write(b.join("y.txt"), b"yyy").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        // Solo "a" existe en el destino.
        fs::create_dir(dest.join("a")).unwrap();
        let dest_root_a = dest.join("a");
        let req = transfer(false, vec![a, b], dest.clone());
        let mut p = plan(&req).unwrap();

        apply_folder_decision(&mut p, &dest_root_a, FolderDecision::Skip);

        // Los pasos de "b" siguen; los de "a" se fueron.
        assert!(!p.steps.iter().any(|s| s.to.starts_with(&dest_root_a)));
        assert!(p
            .steps
            .iter()
            .any(|s| s.to.starts_with(dest.join("b").as_path())));
        // total_files quedó en 1 (solo y.txt).
        assert_eq!(p.total_files, 1);
        assert_eq!(p.total_bytes, 3);
    }

    #[test]
    fn replace_agrega_el_borrado_del_destino() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let mut p = plan(&req).unwrap();
        let pasos_antes = p.steps.clone();

        apply_folder_decision(&mut p, &dest_root, FolderDecision::Replace);

        // Replace agrega EXACTAMENTE dest_root al pre_delete; NO quita pasos de copia.
        assert_eq!(p.pre_delete, vec![dest_root]);
        assert_eq!(p.steps, pasos_antes, "Replace no filtra pasos");
    }

    #[test]
    fn replace_idempotente_no_duplica() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let mut p = plan(&req).unwrap();
        apply_folder_decision(&mut p, &dest_root, FolderDecision::Replace);
        apply_folder_decision(&mut p, &dest_root, FolderDecision::Replace);
        assert_eq!(p.pre_delete, vec![dest_root], "no se duplica el destino");
    }
}

#[cfg(test)]
mod conflict_prompt_tests {
    use super::*;
    use std::fs;

    #[test]
    fn from_paths_trae_los_metadatos_de_ambos_lados() {
        // Dos archivos con TAMAÑOS distintos: el prompt debe traer cada tamaño en su lado y una
        // fecha de modificación presente (Some) para ambos.
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("dst.txt");
        let incoming = dir.path().join("src.txt");
        fs::write(&existing, b"viejo").unwrap(); // 5 bytes
        fs::write(&incoming, b"contenido-nuevo").unwrap(); // 15 bytes

        let p = ConflictPrompt::from_paths(existing.clone(), incoming.clone());
        assert_eq!(p.existing, existing);
        assert_eq!(p.incoming, incoming);
        assert_eq!(p.existing_size, 5, "tamaño del existente");
        assert_eq!(p.incoming_size, 15, "tamaño del entrante");
        assert!(
            !p.existing_is_dir && !p.incoming_is_dir,
            "ambos son archivos"
        );
        assert!(p.existing_modified.is_some(), "fecha del existente leída");
        assert!(p.incoming_modified.is_some(), "fecha del entrante leída");
    }

    #[test]
    fn from_paths_tolerante_a_ruta_inexistente() {
        // Si una ruta no existe, su tamaño es 0 y su fecha None; no falla.
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("existe.txt");
        fs::write(&existing, b"hola").unwrap();
        let incoming = dir.path().join("no_existe.txt"); // nunca creado

        let p = ConflictPrompt::from_paths(existing, incoming);
        assert_eq!(p.existing_size, 4);
        assert!(p.existing_modified.is_some());
        assert_eq!(p.incoming_size, 0, "ruta inexistente → tamaño 0");
        assert!(
            p.incoming_modified.is_none(),
            "ruta inexistente → sin fecha"
        );
    }

    #[test]
    fn from_paths_marca_carpeta_y_tamano_cero() {
        let dir = tempfile::tempdir().unwrap();
        let carpeta = dir.path().join("una_carpeta");
        fs::create_dir(&carpeta).unwrap();
        let archivo = dir.path().join("a.txt");
        fs::write(&archivo, b"xyz").unwrap();

        let p = ConflictPrompt::from_paths(carpeta, archivo);
        assert!(p.existing_is_dir, "el existente es carpeta");
        assert_eq!(p.existing_size, 0, "carpeta → tamaño 0");
        assert!(!p.incoming_is_dir);
    }

    #[test]
    fn are_identical_mismo_tamano_y_fecha_es_true() {
        // Copiar un archivo con std::fs::copy preserva el contenido; igualamos el mtime a mano para
        // un escenario determinista de "idénticos".
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"mismo-contenido").unwrap();
        fs::write(&b, b"mismo-contenido").unwrap();
        // Forzar el MISMO mtime exacto en ambos.
        let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        filetime_set(&a, t);
        filetime_set(&b, t);
        assert!(are_identical(&a, &b), "mismo tamaño y fecha → idénticos");
    }

    #[test]
    fn are_identical_tamano_distinto_es_false() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"corto").unwrap();
        fs::write(&b, b"mucho-mas-largo-que-el-otro").unwrap();
        let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        filetime_set(&a, t);
        filetime_set(&b, t);
        assert!(!are_identical(&a, &b), "tamaños distintos → no idénticos");
    }

    #[test]
    fn are_identical_fecha_lejana_es_false() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"igual").unwrap();
        fs::write(&b, b"igual").unwrap();
        let ta = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let tb = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_100);
        filetime_set(&a, ta);
        filetime_set(&b, tb);
        assert!(
            !are_identical(&a, &b),
            "fechas separadas >2s → no idénticos"
        );
    }

    #[test]
    fn are_identical_tolera_2s_de_diferencia() {
        // FAT guarda mtime con resolución de 2s: una diferencia de hasta 2s sigue siendo idéntico.
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"igual").unwrap();
        fs::write(&b, b"igual").unwrap();
        let ta = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let tb = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_002);
        filetime_set(&a, ta);
        filetime_set(&b, tb);
        assert!(
            are_identical(&a, &b),
            "diferencia de 2s entra en tolerancia"
        );
    }

    #[test]
    fn are_identical_carpeta_es_false() {
        let dir = tempfile::tempdir().unwrap();
        let carpeta = dir.path().join("c");
        fs::create_dir(&carpeta).unwrap();
        let archivo = dir.path().join("a.txt");
        fs::write(&archivo, b"").unwrap();
        assert!(
            !are_identical(&carpeta, &archivo),
            "una carpeta nunca es idéntica por este criterio"
        );
    }

    /// Ajusta el mtime de un archivo SIN depender del crate `filetime` (que no es dependencia de
    /// core): reabre el archivo y usa la API de la plataforma vía `std`. En la práctica, en Windows
    /// y Unix `std` no expone set_mtime; usamos un truco portable: escribir y luego tocar con
    /// `File::set_modified` (estable desde Rust 1.75).
    fn filetime_set(path: &Path, t: std::time::SystemTime) {
        let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(t).unwrap();
    }
}
