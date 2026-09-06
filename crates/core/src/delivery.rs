// Naygo — entregas revisables, con manifiesto y publicación sin sobrescribir.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Todo I/O pertenece al worker. Preparar congela el inventario; ejecutar no vuelve a
//! descubrir archivos. Reutiliza el motor de copia cancelable y nunca mueve originales.

use crate::ops::{ConflictPolicy, OpKind, OpMsg, OpPlan, OpStep};
use crate::CancellationToken;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, Metadata};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{channel, Sender},
};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_ENTRIES: usize = 50_000;
const MANIFEST: &str = "naygo-manifest.json";
const LIST: &str = "naygo-files.txt";
static STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Structure {
    /// Archivos al mismo nivel; rechaza homónimos, incluso si difieren solo en mayúsculas.
    Flat,
    /// Conserva el árbol desde una raíz elegida expresamente.
    Relative(PathBuf),
    /// Cada fuente tiene un grupo numerado; conserva descendientes y carpetas vacías.
    Grouped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Output {
    Folder,
    Zip,
}

/// Opciones explícitas de revisión; nunca modifican los nombres de los originales.
#[derive(Clone, Debug, Default)]
pub struct ReviewOptions {
    /// Un nombre por fuente, en el orden capturado. Vacío conserva los nombres sugeridos.
    pub group_names: Vec<String>,
    /// Solo en estructura plana: propone sufijos numerados y exige revisar de nuevo.
    pub rename_flat_duplicates: bool,
}

#[derive(Clone, Debug)]
pub struct NameConflict {
    pub relative: PathBuf,
    pub sources: Vec<PathBuf>,
}

pub fn suggested_group_names(sources: &[PathBuf]) -> Vec<String> {
    sources
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                "{:02}-{}",
                i + 1,
                p.file_name().unwrap_or_default().to_string_lossy()
            )
        })
        .collect()
}

#[derive(Debug)]
pub enum DeliveryError {
    Cancelled,
    Invalid(&'static str),
    Changed(PathBuf),
    Io(std::io::Error),
    Copy(String),
    Conflicts(Vec<NameConflict>),
}

impl std::fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("cancelled"),
            Self::Invalid(reason) => f.write_str(reason),
            Self::Changed(path) => write!(f, "source changed: {}", path.display()),
            Self::Io(error) => error.fmt(f),
            Self::Copy(error) => f.write_str(error),
            Self::Conflicts(conflicts) => write!(f, "{} conflicting output names", conflicts.len()),
        }
    }
}
impl From<std::io::Error> for DeliveryError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: Option<SystemTime>,
    directory: bool,
}
impl Stamp {
    fn of(metadata: &Metadata) -> Self {
        Self {
            size: if metadata.is_dir() { 0 } else { metadata.len() },
            modified: metadata.modified().ok(),
            directory: metadata.is_dir(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeliveryEntry {
    source: PathBuf,
    pub relative: PathBuf,
    stamp: Stamp,
    sha256: Option<String>,
}
impl DeliveryEntry {
    pub fn source(&self) -> &Path {
        &self.source
    }
    pub fn size(&self) -> u64 {
        self.stamp.size
    }
    pub fn is_dir(&self) -> bool {
        self.stamp.directory
    }
}

/// El plan solo se construye mediante `prepare`; una receta importada no puede inyectar
/// entradas ni destinos ya expandidos. Las rutas relativas se validan antes de usarse.
#[derive(Clone, Debug)]
pub struct DeliveryPlan {
    pub entries: Vec<DeliveryEntry>,
    pub destination: PathBuf,
    pub output: Output,
    pub total_bytes: u64,
}

impl DeliveryPlan {
    /// Lectura adicional solicitada explícitamente. Los hashes de la revisión quedan
    /// congelados y se verifican otra vez antes de copiar y sobre la copia terminada.
    pub fn capture_hashes(&mut self, token: &CancellationToken) -> Result<(), DeliveryError> {
        for entry in &mut self.entries {
            checkpoint(token)?;
            if !entry.is_dir() {
                entry.sha256 = Some(read_hash(&entry.source, token)?);
            }
        }
        revalidate(self, token, false)
    }
}

fn read_hash(path: &Path, token: &CancellationToken) -> Result<String, DeliveryError> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        checkpoint(token)?;
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        digest.update(&buffer[..size]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn checkpoint(token: &CancellationToken) -> Result<(), DeliveryError> {
    token.wait_if_paused();
    if token.is_cancelled() {
        Err(DeliveryError::Cancelled)
    } else {
        Ok(())
    }
}

fn linked(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

fn checked_metadata(path: &Path) -> Result<Metadata, DeliveryError> {
    let metadata = fs::symlink_metadata(path)?;
    if linked(&metadata) || (!metadata.is_file() && !metadata.is_dir()) {
        return Err(DeliveryError::Invalid(
            "links and special files are not delivery sources",
        ));
    }
    Ok(metadata)
}

fn key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|c| match c {
            Component::Normal(name) => name.to_str().is_some_and(crate::ops::names::is_valid_name),
            _ => false,
        })
}

/// Scan acotado, sin seguir links. No crea destino ni parciales durante la revisión.
pub fn prepare(
    sources: &[PathBuf],
    destination: &Path,
    structure: &Structure,
    output: Output,
    token: &CancellationToken,
) -> Result<DeliveryPlan, DeliveryError> {
    prepare_review(
        sources,
        destination,
        structure,
        output,
        &ReviewOptions::default(),
        token,
    )
}

pub fn prepare_review(
    sources: &[PathBuf],
    destination: &Path,
    structure: &Structure,
    output: Output,
    options: &ReviewOptions,
    token: &CancellationToken,
) -> Result<DeliveryPlan, DeliveryError> {
    checkpoint(token)?;
    if sources.is_empty() {
        return Err(DeliveryError::Invalid("empty selection"));
    }
    if sources.len() > MAX_ENTRIES {
        return Err(DeliveryError::Invalid("delivery entry limit exceeded"));
    }
    let group_names = if options.group_names.is_empty() {
        suggested_group_names(sources)
    } else {
        options.group_names.clone()
    };
    if matches!(structure, Structure::Grouped)
        && (group_names.len() != sources.len()
            || group_names.iter().any(|name| {
                !crate::ops::names::is_valid_name(name)
                    || !safe_relative(Path::new(name))
                    || Path::new(name).components().count() != 1
            }))
    {
        return Err(DeliveryError::Invalid(
            "provide one valid group name per source",
        ));
    }
    let name = destination
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| crate::ops::names::is_valid_name(n))
        .ok_or(DeliveryError::Invalid("invalid delivery name"))?;
    let parent = fs::canonicalize(
        destination
            .parent()
            .ok_or(DeliveryError::Invalid("missing destination"))?,
    )?;
    let destination = parent.join(name);
    ensure_absent(&destination)?;
    let mut roots = Vec::new();
    let mut root_keys = BTreeSet::new();
    for source in sources {
        checkpoint(token)?;
        checked_metadata(source)?;
        let source = fs::canonicalize(source)?;
        if !root_keys.insert(key(&source)) {
            return Err(DeliveryError::Invalid("overlapping sources"));
        }
        if key(&destination) == key(&source)
            || key(&destination).starts_with(&(key(&source) + "\\"))
        {
            return Err(DeliveryError::Invalid("destination inside source"));
        }
        roots.push(source);
    }
    // No comparar cada pareja: miles de referencias deben planificarse en O(n log n).
    for source in &root_keys {
        checkpoint(token)?;
        let prefix = source.clone() + "\\";
        if root_keys
            .range(prefix.clone()..)
            .next()
            .is_some_and(|next| next.starts_with(&prefix))
        {
            return Err(DeliveryError::Invalid("overlapping sources"));
        }
    }
    let relative_root = if let Structure::Relative(root) = structure {
        Some(fs::canonicalize(root)?)
    } else {
        None
    };
    let mut stack = Vec::new();
    for (index, source) in roots.iter().enumerate().rev() {
        let name = source.file_name().ok_or(DeliveryError::Invalid(
            "select a folder or file, not a volume root",
        ))?;
        let relative = match structure {
            Structure::Flat => PathBuf::from(name),
            Structure::Grouped => PathBuf::from(&group_names[index]),
            Structure::Relative(_) => source
                .strip_prefix(relative_root.as_ref().unwrap())
                .map_err(|_| DeliveryError::Invalid("source outside selected root"))?
                .to_path_buf(),
        };
        stack.push((source.clone(), relative));
    }
    let mut entries = Vec::new();
    let mut names = HashSet::from([key(Path::new(MANIFEST)), key(Path::new(LIST))]);
    let mut counters = HashMap::new();
    let mut owners: BTreeMap<String, NameConflict> = BTreeMap::new();
    for reserved in [MANIFEST, LIST] {
        owners.insert(
            key(Path::new(reserved)),
            NameConflict {
                relative: PathBuf::from(reserved),
                sources: Vec::new(),
            },
        );
    }
    let mut total_bytes = 0u64;
    let mut visited = 0usize;
    while let Some((source, mut relative)) = stack.pop() {
        checkpoint(token)?;
        visited += 1;
        if visited > MAX_ENTRIES {
            return Err(DeliveryError::Invalid("delivery entry limit exceeded"));
        }
        let metadata = checked_metadata(&source)?;
        if !(relative.as_os_str().is_empty()
            || matches!(structure, Structure::Flat) && metadata.is_dir())
        {
            if !safe_relative(&relative) {
                return Err(DeliveryError::Invalid("invalid relative name"));
            }
            if matches!(structure, Structure::Flat) && options.rename_flat_duplicates {
                relative = unique_flat_name(&relative, &names, &mut counters, token)?;
            }
            names.insert(key(&relative));
            owners
                .entry(key(&relative))
                .or_insert_with(|| NameConflict {
                    relative: relative.clone(),
                    sources: Vec::new(),
                })
                .sources
                .push(source.clone());
            let stamp = Stamp::of(&metadata);
            total_bytes = total_bytes
                .checked_add(stamp.size)
                .ok_or(DeliveryError::Invalid("size overflow"))?;
            entries.push(DeliveryEntry {
                source: source.clone(),
                relative: relative.clone(),
                stamp,
                sha256: None,
            });
        }
        if metadata.is_dir() {
            let mut children = Vec::new();
            for child in fs::read_dir(&source)? {
                checkpoint(token)?;
                if visited + stack.len() + children.len() >= MAX_ENTRIES {
                    return Err(DeliveryError::Invalid("delivery entry limit exceeded"));
                }
                let child = child?;
                let child_relative = if matches!(structure, Structure::Flat) {
                    PathBuf::from(child.file_name())
                } else {
                    relative.join(child.file_name())
                };
                children.push((child.path(), child_relative));
            }
            children.sort_by(|a, b| a.1.cmp(&b.1));
            stack.extend(children.into_iter().rev());
        }
    }
    if entries.is_empty() {
        return Err(DeliveryError::Invalid(
            "no files in flat delivery; use grouped for empty folders",
        ));
    }
    let conflicts: Vec<_> = owners
        .into_values()
        .filter(|owner| {
            owner.sources.len() > 1
                || (!owner.sources.is_empty()
                    && [key(Path::new(MANIFEST)), key(Path::new(LIST))]
                        .contains(&key(&owner.relative)))
        })
        .collect();
    if !conflicts.is_empty() {
        return Err(DeliveryError::Conflicts(conflicts));
    }
    Ok(DeliveryPlan {
        entries,
        destination,
        output,
        total_bytes,
    })
}

fn unique_flat_name(
    candidate: &Path,
    used: &HashSet<String>,
    counters: &mut HashMap<String, u32>,
    token: &CancellationToken,
) -> Result<PathBuf, DeliveryError> {
    if !used.contains(&key(candidate)) {
        return Ok(candidate.to_path_buf());
    }
    let next = counters.entry(key(candidate)).or_insert(2);
    let stem = candidate.file_stem().unwrap_or_default().to_string_lossy();
    loop {
        checkpoint(token)?;
        let suffix = *next;
        *next += 1;
        let name = match candidate.extension() {
            Some(extension) => format!("{stem} ({suffix}).{}", extension.to_string_lossy()),
            None => format!("{stem} ({suffix})"),
        };
        let path = PathBuf::from(name);
        if !used.contains(&key(&path)) {
            return Ok(path);
        }
    }
}

fn ensure_absent(path: &Path) -> Result<(), DeliveryError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(DeliveryError::Invalid(
            "destination already exists; choose a new name",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Temporal exclusivo, creado bajo el padre del destino. Drop solo retira ese directorio
/// creado por esta ejecución; nunca recibe rutas de usuario ni borra el destino publicado.
struct Stage(PathBuf);
impl Stage {
    fn new(parent: &Path) -> Result<Self, DeliveryError> {
        for _ in 0..100 {
            let id = STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".naygo-delivery-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(DeliveryError::Invalid("cannot reserve delivery staging"))
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_dir_all(&self.0) {
            tracing::warn!("delivery staging cleanup failed: {}: {e}", self.0.display());
        }
    }
}

fn revalidate(
    plan: &DeliveryPlan,
    token: &CancellationToken,
    content: bool,
) -> Result<(), DeliveryError> {
    for entry in &plan.entries {
        checkpoint(token)?;
        if !safe_relative(&entry.relative)
            || Stamp::of(&checked_metadata(&entry.source)?) != entry.stamp
        {
            return Err(DeliveryError::Changed(entry.source.clone()));
        }
        if content {
            if let Some(expected) = &entry.sha256 {
                if read_hash(&entry.source, token)? != *expected {
                    return Err(DeliveryError::Changed(entry.source.clone()));
                }
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct Manifest<'a> {
    version: u32,
    created_utc: u64,
    verification: &'static str,
    entries: Vec<ManifestEntry<'a>>,
}
#[derive(Serialize)]
struct ManifestEntry<'a> {
    path: &'a Path,
    bytes: u64,
    directory: bool,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<&'a str>,
}

/// Publica solo si TODOS los pasos terminan bien. Una copia cancelada/fallida no produce
/// una entrega parcial. El manifiesto no incluye rutas absolutas de los originales.
pub fn execute(
    plan: &DeliveryPlan,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
) -> Result<PathBuf, DeliveryError> {
    checkpoint(token)?;
    ensure_absent(&plan.destination)?;
    revalidate(plan, token, true)?;
    let stage = Stage::new(
        plan.destination
            .parent()
            .ok_or(DeliveryError::Invalid("missing destination"))?,
    )?;
    let content = stage.0.join("content");
    fs::create_dir(&content)?;
    let copy_plan = OpPlan {
        steps: plan
            .entries
            .iter()
            .map(|entry| OpStep {
                from: Some(entry.source.clone()),
                to: content.join(&entry.relative),
                bytes: entry.size(),
                is_dir: entry.is_dir(),
            })
            .collect(),
        total_bytes: plan.total_bytes,
        total_files: plan.entries.iter().filter(|e| !e.is_dir()).count(),
        pre_delete: Vec::new(),
    };
    let (_conflict_tx, conflict_rx) = channel();
    let summary = crate::ops::engine::run_plan(
        &copy_plan,
        &OpKind::Copy,
        ConflictPolicy::Skip,
        token,
        tx,
        &conflict_rx,
        None,
    );
    checkpoint(token)?;
    if summary.count_failed() != 0
        || summary.count_skipped() != 0
        || summary.count_done() != plan.entries.len()
    {
        let detail = summary.items.iter().find_map(|i| {
            if let crate::ops::OpOutcome::Failed(e) = &i.outcome {
                Some(e.clone())
            } else {
                None
            }
        });
        return Err(DeliveryError::Copy(
            detail.unwrap_or_else(|| "incomplete delivery copy".into()),
        ));
    }
    revalidate(plan, token, false)?;
    for entry in &plan.entries {
        checkpoint(token)?;
        if let Some(expected) = &entry.sha256 {
            if read_hash(&content.join(&entry.relative), token)? != *expected {
                return Err(DeliveryError::Changed(entry.source.clone()));
            }
        }
    }
    let manifest = Manifest {
        version: 1,
        created_utc: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        verification: if plan.entries.iter().any(|e| e.sha256.is_some()) {
            "SHA-256"
        } else {
            "size-and-modified-time (not a content hash)"
        },
        entries: plan
            .entries
            .iter()
            .map(|entry| ManifestEntry {
                path: &entry.relative,
                bytes: entry.size(),
                directory: entry.is_dir(),
                status: "copied",
                sha256: entry.sha256.as_deref(),
            })
            .collect(),
    };
    fs::write(
        content.join(MANIFEST),
        serde_json::to_vec_pretty(&manifest).map_err(|e| DeliveryError::Copy(e.to_string()))?,
    )?;
    let listing = plan
        .entries
        .iter()
        .map(|entry| format!("{}\t{}\n", entry.relative.display(), entry.size()))
        .collect::<String>();
    fs::write(content.join(LIST), listing)?;
    checkpoint(token)?;
    let publish = match plan.output {
        Output::Folder => content,
        Output::Zip => {
            let zip = stage.0.join("delivery.zip");
            let mut entries: Vec<_> = plan
                .entries
                .iter()
                .map(|entry| {
                    (
                        content.join(&entry.relative),
                        entry.relative.to_string_lossy().replace('\\', "/"),
                    )
                })
                .collect();
            entries.extend(
                [MANIFEST, LIST]
                    .into_iter()
                    .map(|name| (content.join(name), name.to_string())),
            );
            let mut progress = |done, total| {
                let _ = tx.send(OpMsg::Progress(crate::ops::OpProgress {
                    bytes_done: done,
                    bytes_total: total,
                    files_done: 0,
                    files_total: 0,
                    current: plan.destination.clone(),
                }));
            };
            let results =
                crate::archive_ops::compress_zip_entries(&entries, &zip, &mut progress, token)
                    .map_err(|e| {
                        if token.is_cancelled() {
                            DeliveryError::Cancelled
                        } else {
                            DeliveryError::Copy(e.to_string())
                        }
                    })?;
            if results
                .iter()
                .any(|r| !matches!(r.outcome, crate::archive_ops::ArchiveOutcome::Done))
            {
                return Err(DeliveryError::Invalid("incomplete ZIP"));
            }
            zip
        }
    };
    checkpoint(token)?;
    ensure_absent(&plan.destination)?;
    // Windows rename falla si apareció un destino entre la comprobación y el rename.
    fs::rename(publish, &plan.destination)?;
    Ok(plan.destination.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_verifica_contenido_y_detecta_cambio_con_metadata_identica() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("a.txt");
        fs::write(&source, b"abc").unwrap();
        let token = CancellationToken::new();
        let mut plan = prepare(
            std::slice::from_ref(&source),
            &temp.path().join("delivery"),
            &Structure::Flat,
            Output::Folder,
            &token,
        )
        .unwrap();
        plan.capture_hashes(&token).unwrap();
        let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(plan.entries[0].sha256.as_deref(), Some(hash));
        let (tx, _) = channel();
        execute(&plan, &token, &tx).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(plan.destination.join(MANIFEST)).unwrap()).unwrap();
        assert_eq!(manifest["verification"], "SHA-256");
        assert_eq!(manifest["entries"][0]["sha256"], hash);
        let stamp = fs::metadata(&source).unwrap().modified().unwrap();
        fs::write(&source, b"xyz").unwrap();
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(stamp))
            .unwrap();
        plan.destination = temp.path().join("another-delivery");
        assert!(matches!(
            execute(&plan, &token, &tx),
            Err(DeliveryError::Changed(_))
        ));
        assert!(!plan.destination.exists());
    }

    #[test]
    fn entrega_carpeta_relativa_preserva_originales_y_manifiesto_sin_rutas_privadas() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("private-source");
        fs::create_dir_all(source.join("sub/empty")).unwrap();
        fs::write(source.join("sub/a.txt"), b"original").unwrap();
        let destination = temp.path().join("delivery");
        let token = CancellationToken::new();
        let plan = prepare(
            std::slice::from_ref(&source),
            &destination,
            &Structure::Relative(source.clone()),
            Output::Folder,
            &token,
        )
        .unwrap();
        assert!(!destination.exists());
        let (tx, _) = channel();
        execute(&plan, &token, &tx).unwrap();
        assert_eq!(
            fs::read(destination.join("sub/a.txt")).unwrap(),
            b"original"
        );
        assert_eq!(fs::read(source.join("sub/a.txt")).unwrap(), b"original");
        assert!(destination.join("sub/empty").is_dir());
        let manifest = fs::read_to_string(destination.join(MANIFEST)).unwrap();
        assert!(!manifest.contains("private-source"));
        assert!(manifest.contains("not a content hash"));
        assert!(destination.join(LIST).is_file());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
    }

    #[test]
    fn homonimos_no_se_pisan_y_grupos_los_desambiguan() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("a")).unwrap();
        fs::create_dir(temp.path().join("b")).unwrap();
        let sources = [
            temp.path().join("a/info.txt"),
            temp.path().join("b/INFO.txt"),
        ];
        for path in &sources {
            fs::write(path, b"content").unwrap();
        }
        let token = CancellationToken::new();
        let dest = temp.path().join("out.zip");
        assert!(prepare(&sources, &dest, &Structure::Flat, Output::Zip, &token).is_err());
        let plan = prepare(&sources, &dest, &Structure::Grouped, Output::Zip, &token).unwrap();
        let (tx, _) = channel();
        execute(&plan, &token, &tx).unwrap();
        let mut zip = zip::ZipArchive::new(fs::File::open(&dest).unwrap()).unwrap();
        assert!(zip.by_name("01-info.txt").is_ok());
        assert!(zip.by_name("02-INFO.txt").is_ok());
        assert!(zip.by_name(MANIFEST).is_ok());
        assert!(zip.by_name(LIST).is_ok());
        assert_eq!(zip.len(), 4);
    }

    #[test]
    fn fuentes_congeladas_cambio_de_origen_no_publica() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("input.txt");
        fs::write(&source, b"first").unwrap();
        let token = CancellationToken::new();
        let dest = temp.path().join("out");
        let plan = prepare(
            std::slice::from_ref(&source),
            &dest,
            &Structure::Flat,
            Output::Folder,
            &token,
        )
        .unwrap();
        fs::write(&source, b"longer replacement").unwrap();
        let (tx, _) = channel();
        assert!(matches!(
            execute(&plan, &token, &tx),
            Err(DeliveryError::Changed(_))
        ));
        assert!(!dest.exists());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[test]
    fn destino_aparecido_tras_revision_se_conserva() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("input.txt");
        fs::write(&source, b"first").unwrap();
        let token = CancellationToken::new();
        let dest = temp.path().join("out.zip");
        let plan = prepare(&[source], &dest, &Structure::Flat, Output::Zip, &token).unwrap();
        fs::write(&dest, b"another application created this").unwrap();
        let (tx, _) = channel();
        assert!(execute(&plan, &token, &tx).is_err());
        assert_eq!(fs::read(dest).unwrap(), b"another application created this");
    }

    #[test]
    fn cancelacion_y_destinos_recursivos_no_crean_parciales() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("a"), b"first").unwrap();
        let token = CancellationToken::new();
        assert!(prepare(
            std::slice::from_ref(&source),
            &source.join("delivery"),
            &Structure::Grouped,
            Output::Folder,
            &token
        )
        .is_err());
        assert!(prepare(
            &[source.clone(), source.join("a")],
            &temp.path().join("out"),
            &Structure::Grouped,
            Output::Folder,
            &token
        )
        .is_err());
        let plan = prepare(
            &[source],
            &temp.path().join("out"),
            &Structure::Grouped,
            Output::Folder,
            &token,
        )
        .unwrap();
        token.cancel();
        let (tx, _) = channel();
        assert!(matches!(
            execute(&plan, &token, &tx),
            Err(DeliveryError::Cancelled)
        ));
        assert!(!plan.destination.exists());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[test]
    fn protege_manifiesto_y_raiz_explicita() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join(MANIFEST);
        fs::write(&source, b"do not overwrite").unwrap();
        let token = CancellationToken::new();
        assert!(prepare(
            std::slice::from_ref(&source),
            &temp.path().join("out"),
            &Structure::Flat,
            Output::Folder,
            &token
        )
        .is_err());
        fs::create_dir(temp.path().join("other")).unwrap();
        assert!(prepare(
            &[source],
            &temp.path().join("out"),
            &Structure::Relative(temp.path().join("other")),
            Output::Folder,
            &token
        )
        .is_err());
        assert!(!safe_relative(Path::new("../escape")));
        assert!(!safe_relative(Path::new("C:/absolute")));
    }

    #[test]
    fn revision_reune_homonimos_y_propone_nombres_sin_perder_archivos() {
        let temp = tempfile::tempdir().unwrap();
        let mut sources = Vec::new();
        for group in ["a", "b", "c"] {
            let dir = temp.path().join(group);
            fs::create_dir(&dir).unwrap();
            for name in ["info.txt", "data.csv"] {
                let path = dir.join(name);
                fs::write(&path, group).unwrap();
                sources.push(path);
            }
        }
        let token = CancellationToken::new();
        let dest = temp.path().join("out");
        let error = prepare(&sources, &dest, &Structure::Flat, Output::Folder, &token).unwrap_err();
        let DeliveryError::Conflicts(conflicts) = error else {
            panic!("expected conflicts");
        };
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts.iter().all(|c| c.sources.len() == 3));
        assert!(!dest.exists());
        let options = ReviewOptions {
            rename_flat_duplicates: true,
            ..Default::default()
        };
        let plan = prepare_review(
            &sources,
            &dest,
            &Structure::Flat,
            Output::Folder,
            &options,
            &token,
        )
        .unwrap();
        assert_eq!(plan.entries.len(), sources.len());
        let names: HashSet<_> = plan.entries.iter().map(|e| e.relative.clone()).collect();
        assert_eq!(names.len(), 6);
        assert!(names.contains(Path::new("info (3).txt")));
        assert_eq!(
            plan.entries[4].source(),
            fs::canonicalize(&sources[4]).unwrap()
        );
        let (tx, _) = channel();
        execute(&plan, &token, &tx).unwrap();
        assert_eq!(fs::read_to_string(dest.join("info (3).txt")).unwrap(), "c");
        assert!(sources.iter().all(|p| p.exists()));
    }

    #[test]
    fn grupos_editables_conservan_descendientes_y_rechazan_colisiones() {
        let temp = tempfile::tempdir().unwrap();
        let mut sources = Vec::new();
        for name in ["one", "two"] {
            let root = temp.path().join(name);
            fs::create_dir(&root).unwrap();
            fs::write(root.join("info.txt"), name).unwrap();
            sources.push(root);
        }
        let token = CancellationToken::new();
        let destination = temp.path().join("out.zip");
        let mut options = ReviewOptions {
            group_names: vec!["Cliente A".into(), "Cliente B".into()],
            ..Default::default()
        };
        let plan = prepare_review(
            &sources,
            &destination,
            &Structure::Grouped,
            Output::Zip,
            &options,
            &token,
        )
        .unwrap();
        assert!(plan
            .entries
            .iter()
            .any(|e| e.relative == Path::new("Cliente A/info.txt")));
        options.group_names[1] = "cliente a".into();
        assert!(matches!(
            prepare_review(
                &sources,
                &destination,
                &Structure::Grouped,
                Output::Zip,
                &options,
                &token
            ),
            Err(DeliveryError::Conflicts(_))
        ));
        for invalid in ["../outside", "x/y", "foo/", "foo\\", "", "bad.", "bad "] {
            options.group_names[0] = invalid.into();
            assert!(prepare_review(
                &sources,
                &destination,
                &Structure::Grouped,
                Output::Zip,
                &options,
                &token
            )
            .is_err());
        }
        options.group_names = vec!["only one".into()];
        assert!(prepare_review(
            &sources,
            &destination,
            &Structure::Grouped,
            Output::Zip,
            &options,
            &token
        )
        .is_err());
        assert!(!destination.exists());
    }

    #[test]
    fn renombrado_plano_respeta_reservados_sufijos_y_cancelacion() {
        let used: HashSet<_> = ["info.txt", "info (2).txt", MANIFEST]
            .iter()
            .map(|p| key(Path::new(p)))
            .collect();
        let mut counters = HashMap::new();
        let token = CancellationToken::new();
        assert_eq!(
            unique_flat_name(Path::new("INFO.txt"), &used, &mut counters, &token).unwrap(),
            Path::new("INFO (3).txt")
        );
        assert_eq!(
            unique_flat_name(Path::new(MANIFEST), &used, &mut counters, &token).unwrap(),
            Path::new("naygo-manifest (2).json")
        );
        token.cancel();
        assert!(matches!(
            unique_flat_name(Path::new("info.txt"), &used, &mut counters, &token),
            Err(DeliveryError::Cancelled)
        ));
    }
}
