// Naygo — import/export de "packs" (idiomas, temas y configuración) como archivos .zip.
// Un pack es un .zip con uno o más JSON del directorio de configuración:
//   - idioma:  lang/<code>.json
//   - tema:    themes/<id>.json
//   - config:  settings.json (+ keybindings.json si existe)
// El import detecta el tipo por la(s) entrada(s) del zip y extrae a la subcarpeta correcta del
// directorio de configuración. Funciones puras (toman rutas explícitas) → testeables.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use naygo_platform::dir_watch::Waker;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::Instant;
use zip::write::SimpleFileOptions;

/// Trabajo de empaquetado enviado desde callbacks. Sólo contiene snapshots propios; el worker no
/// recibe controladores, modelos Slint ni préstamos del event loop.
pub(crate) enum PackJob {
    LoadCatalogs {
        config_dir: PathBuf,
        data_dir: PathBuf,
        active_theme: String,
    },
    ImportTopic {
        config_dir: PathBuf,
        data_dir: PathBuf,
        input: PathBuf,
        active_language: String,
    },
    ExportLang {
        config_dir: PathBuf,
        code: String,
        output: PathBuf,
    },
    ExportTheme {
        config_dir: PathBuf,
        data_dir: PathBuf,
        id: String,
        output: PathBuf,
    },
    ExportConfig {
        config_dir: PathBuf,
        output: PathBuf,
    },
    SaveTheme {
        config_dir: PathBuf,
        id: String,
        old_id: Option<String>,
        json: String,
    },
    DeleteTheme {
        config_dir: PathBuf,
        id: String,
    },
    ImportIcon {
        data_dir: PathBuf,
        legacy_config_dir: PathBuf,
        input: PathBuf,
    },
    CopyUserPng {
        data_dir: PathBuf,
        legacy_config_dir: PathBuf,
        source: PathBuf,
        icon_key: String,
    },
    ExportIcon {
        output: PathBuf,
        name: String,
        author: String,
        base_set_id: String,
        overrides: std::collections::BTreeMap<String, naygo_core::icon_source::IconSource>,
        data_dir: PathBuf,
        legacy_config_dir: PathBuf,
    },
}

struct QueuedPackJob {
    job: PackJob,
    queued_at: Option<Instant>,
}

/// Resultado aplicado en el hilo de UI; importa distinguishes el tipo y exporta conserva el
/// contexto necesario para mostrar el mensaje correcto.
pub(crate) enum PackOutcome {
    CatalogsLoaded {
        result: Result<
            (
                naygo_core::theme::ThemeCatalog,
                naygo_core::icon_set::IconSetCatalog,
            ),
            String,
        >,
    },
    ImportTopic(Result<ImportedTopic, String>),
    ExportTopic(Result<(), String>),
    SaveTheme(Result<(String, naygo_core::theme::ThemeCatalog), String>),
    DeleteTheme {
        id: String,
        result: Result<(bool, naygo_core::theme::ThemeCatalog), String>,
    },
    ImportIcon(
        Result<
            (
                naygo_core::icon_pack::PackManifest,
                naygo_core::icon_set::IconSetCatalog,
            ),
            String,
        >,
    ),
    CopyUserPng {
        icon_key: String,
        result: Result<String, String>,
    },
    ExportIcon(Result<(), String>),
}

pub(crate) type PackOutcomeCallback =
    std::rc::Rc<std::cell::RefCell<Option<std::rc::Rc<dyn Fn(PackOutcome)>>>>;

pub(crate) struct ImportedTopic {
    pub data: ImportedTopicData,
    pub skipped_platform_settings: Vec<SkippedPlatformSetting>,
}

/// Ajustes ignorados al importar configuración para una plataforma que no puede aplicarlos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SkippedPlatformSetting {
    HomePath,
    RecentPaths,
    Autostart,
    GlobalHotkey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfigTarget {
    Windows,
    Linux,
    Other,
}

impl ConfigTarget {
    fn current() -> Self {
        match std::env::consts::OS {
            "windows" => Self::Windows,
            "linux" => Self::Linux,
            _ => Self::Other,
        }
    }
}

struct ImportReport {
    kind: ImportKind,
    skipped_platform_settings: Vec<SkippedPlatformSetting>,
}

pub(crate) enum ImportedTopicData {
    Language(naygo_core::i18n::I18n),
    Theme(
        naygo_core::theme::ThemeCatalog,
        naygo_core::icon_set::IconSetCatalog,
    ),
    Config(Box<crate::config_ctrl::ConfigCtrl>),
}

/// Worker serial y acotado para lectura, compresión, escritura y extracción de packs.
pub(crate) struct PackIo {
    tx: Option<SyncSender<QueuedPackJob>>,
    rx: Receiver<PackOutcome>,
}

impl PackIo {
    pub(crate) fn new(waker: Waker) -> Self {
        let (tx, jobs) = std::sync::mpsc::sync_channel::<QueuedPackJob>(2);
        let (results, rx) = std::sync::mpsc::sync_channel::<PackOutcome>(2);
        let started = std::thread::Builder::new()
            .name("naygo-pack-io".into())
            .spawn(move || {
                while let Ok(queued) = jobs.recv() {
                    let _lease = crate::metrics::job_started(crate::metrics::JobKind::Pack);
                    if let Some(queued_at) = queued.queued_at {
                        crate::metrics::record("pack.queue_wait", queued_at.elapsed());
                    }
                    let started = crate::metrics::queue_timestamp();
                    let outcome = match queued.job {
                        PackJob::LoadCatalogs {
                            config_dir,
                            data_dir,
                            active_theme,
                        } => PackOutcome::CatalogsLoaded {
                            result: Ok((
                                naygo_core::theme::ThemeCatalog::load(
                                    &config_dir,
                                    &naygo_core::theme::ThemeId::new(&active_theme),
                                ),
                                naygo_core::icon_set::IconSetCatalog::load_with_legacy(
                                    &data_dir,
                                    Some(&config_dir),
                                ),
                            )),
                        },
                        PackJob::ImportTopic {
                            config_dir,
                            data_dir,
                            input,
                            active_language,
                        } => {
                            let loaded = import_zip_with_data_report(
                                &config_dir,
                                &data_dir,
                                &input,
                                ConfigTarget::current(),
                            )
                            .map(|report| {
                                let skipped_platform_settings = report.skipped_platform_settings;
                                let kind = report.kind;
                                let data = match &kind {
                                    ImportKind::Lang(_) => {
                                        ImportedTopicData::Language(naygo_core::i18n::I18n::load(
                                            &config_dir,
                                            &naygo_core::i18n::LangId::new(&active_language),
                                        ))
                                    }
                                    ImportKind::Theme(_) => ImportedTopicData::Theme(
                                        naygo_core::theme::ThemeCatalog::load(
                                            &config_dir,
                                            &naygo_core::theme::ThemeCatalog::default_id(),
                                        ),
                                        naygo_core::icon_set::IconSetCatalog::load_with_legacy(
                                            &data_dir,
                                            Some(&config_dir),
                                        ),
                                    ),
                                    ImportKind::Config => ImportedTopicData::Config(Box::new(
                                        crate::config_ctrl::ConfigCtrl::new_with_data_dir(
                                            config_dir.clone(),
                                            data_dir.clone(),
                                        ),
                                    )),
                                };
                                ImportedTopic {
                                    data,
                                    skipped_platform_settings,
                                }
                            });
                            PackOutcome::ImportTopic(loaded)
                        }
                        PackJob::ExportLang {
                            config_dir,
                            code,
                            output,
                        } => PackOutcome::ExportTopic(export_lang(&config_dir, &code, &output)),
                        PackJob::ExportTheme {
                            config_dir,
                            data_dir,
                            id,
                            output,
                        } => PackOutcome::ExportTopic(export_theme_with_data(
                            &config_dir,
                            &data_dir,
                            &id,
                            &output,
                        )),
                        PackJob::ExportConfig { config_dir, output } => {
                            PackOutcome::ExportTopic(export_config(&config_dir, &output))
                        }
                        PackJob::SaveTheme {
                            config_dir,
                            id,
                            old_id,
                            json,
                        } => {
                            let theme_dir = config_dir.join("themes");
                            let result = std::fs::create_dir_all(&theme_dir)
                                .and_then(|()| {
                                    naygo_core::fs_util::write_atomic(
                                        &theme_dir.join(format!("{id}.json")),
                                        &json,
                                    )
                                })
                                .map_err(|error| format!("no se pudo guardar el tema: {error}"))
                                .map(|()| {
                                    if let Some(old_id) = old_id {
                                        let _ = std::fs::remove_file(
                                            theme_dir.join(format!("{old_id}.json")),
                                        );
                                    }
                                    let theme_id = naygo_core::theme::ThemeId::new(&id);
                                    let catalog = naygo_core::theme::ThemeCatalog::load(
                                        &config_dir,
                                        &theme_id,
                                    );
                                    (id, catalog)
                                });
                            PackOutcome::SaveTheme(result)
                        }
                        PackJob::DeleteTheme { config_dir, id } => {
                            let path = config_dir.join("themes").join(format!("{id}.json"));
                            let result = std::fs::remove_file(path)
                                .map(|()| true)
                                .or_else(|error| {
                                    if error.kind() == std::io::ErrorKind::NotFound {
                                        Ok(false)
                                    } else {
                                        Err(error)
                                    }
                                })
                                .map(|removed| {
                                    let catalog = naygo_core::theme::ThemeCatalog::load(
                                        &config_dir,
                                        &naygo_core::theme::ThemeCatalog::default_id(),
                                    );
                                    (removed, catalog)
                                })
                                .map_err(|error| format!("no se pudo eliminar el tema: {error}"));
                            PackOutcome::DeleteTheme { id, result }
                        }
                        PackJob::ImportIcon {
                            data_dir,
                            legacy_config_dir,
                            input,
                        } => {
                            let imported = naygo_core::icon_pack::import_pack(&input, &data_dir)
                                .map(|manifest| {
                                    let catalog =
                                        naygo_core::icon_set::IconSetCatalog::load_with_legacy(
                                            &data_dir,
                                            Some(&legacy_config_dir),
                                        );
                                    (manifest, catalog)
                                });
                            PackOutcome::ImportIcon(imported)
                        }
                        PackJob::CopyUserPng {
                            data_dir,
                            legacy_config_dir,
                            source,
                            icon_key,
                        } => PackOutcome::CopyUserPng {
                            icon_key,
                            result: copy_user_png(&data_dir, &legacy_config_dir, &source),
                        },
                        PackJob::ExportIcon {
                            output,
                            name,
                            author,
                            base_set_id,
                            overrides,
                            data_dir,
                            legacy_config_dir,
                        } => {
                            PackOutcome::ExportIcon(naygo_core::icon_pack::export_pack_with_legacy(
                                &output,
                                &name,
                                &author,
                                &base_set_id,
                                &overrides,
                                &data_dir,
                                &legacy_config_dir,
                            ))
                        }
                    };
                    if let Some(started) = started {
                        crate::metrics::record("pack.run", started.elapsed());
                    }
                    if results.send(outcome).is_err() {
                        break;
                    }
                    waker();
                }
            })
            .is_ok();
        Self {
            tx: started.then_some(tx),
            rx,
        }
    }

    /// Encola sin bloquear la UI; el usuario puede volver a intentarlo si la cola está llena.
    pub(crate) fn submit(&self, job: PackJob) -> Result<(), String> {
        crate::metrics::job_submitted(crate::metrics::JobKind::Pack);
        let queued = QueuedPackJob {
            job,
            queued_at: crate::metrics::queue_timestamp(),
        };
        let result = match self.tx.as_ref() {
            Some(tx) => tx.try_send(queued).map_err(|error| match error {
                TrySendError::Full(_) => "ya hay packs pendientes; espera a que terminen".into(),
                TrySendError::Disconnected(_) => "el worker de packs no está disponible".into(),
            }),
            None => Err("no se pudo iniciar el worker de packs".into()),
        };
        if result.is_err() {
            crate::metrics::job_rejected(crate::metrics::JobKind::Pack);
        }
        result
    }

    /// Drena resultados listos sin esperar por el worker.
    pub(crate) fn drain(&self) -> Vec<PackOutcome> {
        self.rx.try_iter().collect()
    }
}

/// Qué se importó (para que la UI recargue el catálogo correcto).
#[derive(Debug, PartialEq, Eq)]
pub enum ImportKind {
    Lang(String),
    Theme(String),
    Config,
}

fn opts() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
}

fn copy_user_png(
    data_dir: &Path,
    legacy_config_dir: &Path,
    source: &Path,
) -> Result<String, String> {
    let dest_dir = data_dir.join("icons").join("_user");
    let legacy_dir = legacy_config_dir.join("icons").join("_user");
    std::fs::create_dir_all(&dest_dir)
        .map_err(|error| format!("no se pudo crear el directorio de íconos: {error}"))?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("icon");
    let stem: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    let mut rel = format!("{stem}.png");
    for i in 1u32.. {
        let data_exists = dest_dir
            .join(&rel)
            .try_exists()
            .map_err(|error| format!("no se pudo comprobar el nombre del ícono: {error}"))?;
        let legacy_exists = legacy_dir
            .join(&rel)
            .try_exists()
            .map_err(|error| format!("no se pudo comprobar el nombre del ícono: {error}"))?;
        if !data_exists && !legacy_exists {
            break;
        }
        rel = format!("{stem}_{i}.png");
        if i > 9999 {
            return Err("hay demasiados íconos con el mismo nombre".into());
        }
    }
    std::fs::copy(source, dest_dir.join(&rel))
        .map_err(|error| format!("no se pudo copiar el PNG: {error}"))?;
    Ok(rel)
}

/// Escribe un .zip en `out_zip` con cada (nombre-en-zip, ruta-en-disco). Omite las que no
/// existen. Error si no se puede crear el zip o no había nada que empaquetar.
fn zip_files(out_zip: &Path, entries: &[(String, std::path::PathBuf)]) -> Result<(), String> {
    let file =
        std::fs::File::create(out_zip).map_err(|e| format!("no se pudo crear el zip: {e}"))?;
    let mut zw = zip::ZipWriter::new(file);
    let mut wrote = 0;
    for (name, path) in entries {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        zw.start_file(name.clone(), opts())
            .map_err(|e| format!("error al escribir el zip: {e}"))?;
        zw.write_all(&bytes)
            .map_err(|e| format!("error al escribir {name}: {e}"))?;
        wrote += 1;
    }
    zw.finish()
        .map_err(|e| format!("error al cerrar el zip: {e}"))?;
    if wrote == 0 {
        let _ = std::fs::remove_file(out_zip);
        return Err("no había archivos para exportar".to_string());
    }
    Ok(())
}

/// Exporta el idioma `code` (lang/<code>.json) a `out_zip`.
pub fn export_lang(config_dir: &Path, code: &str, out_zip: &Path) -> Result<(), String> {
    let rel = format!("lang/{code}.json");
    zip_files(out_zip, &[(rel.clone(), config_dir.join(&rel))])
}

/// Exporta el tema `id` (themes/<id>.json) a `out_zip`. Los temas embebidos no tienen archivo
/// en disco; en ese caso el export falla con un mensaje claro (el usuario solo puede exportar
/// temas que viven como archivo en su carpeta de config). Si el tema tiene un set de íconos
/// propio en `<config_dir>/icons/<id>/`, sus PNGs se incluyen como `icons/<id>/<name>.png` (6E).
#[cfg(test)]
pub fn export_theme(config_dir: &Path, id: &str, out_zip: &Path) -> Result<(), String> {
    export_theme_with_data(config_dir, config_dir, id, out_zip)
}

/// Variante Linux: los temas siguen en config, mientras que los íconos nuevos viven en data.
/// Los archivos antiguos en config se usan como fallback y data tiene precedencia por nombre.
pub fn export_theme_with_data(
    config_dir: &Path,
    data_dir: &Path,
    id: &str,
    out_zip: &Path,
) -> Result<(), String> {
    let rel = format!("themes/{id}.json");
    let src = config_dir.join(&rel);
    if !src.exists() {
        return Err(format!(
            "el tema «{id}» es embebido (no hay archivo que exportar)"
        ));
    }
    let mut entries = vec![(rel, src)];
    // Adjuntar PNGs; insertar legacy primero y data después para que el zip use data al duplicar.
    let mut icon_files = std::collections::BTreeMap::<String, PathBuf>::new();
    for root in [config_dir, data_dir] {
        let icons_dir = root.join("icons").join(id);
        if let Ok(rd) = std::fs::read_dir(&icons_dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                let is_png = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("png"))
                    .unwrap_or(false);
                if path.is_file() && is_png {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        icon_files.insert(name.to_string(), path);
                    }
                }
            }
        }
    }
    for (name, path) in icon_files {
        entries.push((format!("icons/{id}/{name}"), path));
    }
    zip_files(out_zip, &entries)
}

/// Exporta la configuración (settings.json + keybindings.json si existe) a `out_zip`.
pub fn export_config(config_dir: &Path, out_zip: &Path) -> Result<(), String> {
    zip_files(
        out_zip,
        &[
            (
                "settings.json".to_string(),
                config_dir.join("settings.json"),
            ),
            (
                "keybindings.json".to_string(),
                config_dir.join("keybindings.json"),
            ),
        ],
    )
}

/// Importa un pack `.zip`: detecta el tipo por sus entradas y extrae cada archivo a la
/// subcarpeta correcta de `config_dir`. Valida que cada JSON parsee (no escribe basura).
/// Devuelve qué tipo se importó para que la UI recargue el catálogo.
#[cfg(test)]
pub fn import_zip(config_dir: &Path, in_zip: &Path) -> Result<ImportKind, String> {
    import_zip_with_data(config_dir, config_dir, in_zip)
}

/// Importa JSON en config e íconos PNG en data, manteniendo compatibilidad con packs antiguos.
#[cfg(test)]
fn import_zip_with_data(
    config_dir: &Path,
    data_dir: &Path,
    in_zip: &Path,
) -> Result<ImportKind, String> {
    import_zip_with_data_report(config_dir, data_dir, in_zip, ConfigTarget::current())
        .map(|report| report.kind)
}

fn import_zip_with_data_report(
    config_dir: &Path,
    data_dir: &Path,
    in_zip: &Path,
    target: ConfigTarget,
) -> Result<ImportReport, String> {
    let file = std::fs::File::open(in_zip).map_err(|e| format!("no se pudo abrir el zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip inválido: {e}"))?;

    let mut kind: Option<ImportKind> = None;
    let mut skipped_platform_settings = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("entrada {i} ilegible: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        // Solo aceptamos rutas conocidas y sin escapes (../).
        if name.contains("..") {
            return Err(format!("ruta sospechosa en el pack: {name}"));
        }
        let (subdir, this_kind) = classify(&name)?;
        // Los PNGs (íconos de un tema) son binarios: se validan por la firma, no como JSON.
        // El resto son JSON de config (lang/themes/settings): se validan parseándolos.
        if name.to_ascii_lowercase().ends_with(".png") {
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|e| format!("no se pudo leer {name}: {e}"))?;
            if !is_png(&bytes) {
                return Err(format!("{name} no es un PNG válido"));
            }
            write_entry(data_dir, subdir, &name, &bytes)?;
        } else {
            let mut contents = String::new();
            entry
                .read_to_string(&mut contents)
                .map_err(|e| format!("no se pudo leer {name}: {e}"))?;
            let bytes = if name == "settings.json" {
                let mut settings = serde_json::from_str::<serde_json::Value>(&contents)
                    .map_err(|_| format!("{name} no es JSON válido"))?;
                skipped_platform_settings.extend(sanitize_imported_settings(&mut settings, target));
                serde_json::to_vec(&settings)
                    .map_err(|error| format!("no se pudo preparar {name}: {error}"))?
            } else {
                serde_json::from_str::<serde_json::Value>(&contents)
                    .map_err(|_| format!("{name} no es JSON válido"))?;
                contents.into_bytes()
            };
            write_entry(config_dir, subdir, &name, &bytes)?;
        }
        // El primer archivo decide el tipo informado (un pack de config trae 2; un tema con
        // íconos trae el .json primero, así que el tipo Theme prevalece sobre los PNGs).
        if kind.is_none() || matches!(this_kind, ImportKind::Theme(_)) {
            kind = Some(this_kind);
        }
    }
    Ok(ImportReport {
        kind: kind
            .ok_or_else(|| "el pack está vacío o no tiene archivos reconocidos".to_string())?,
        skipped_platform_settings,
    })
}

/// Filtra solo valores cuyo significado depende de una capacidad o formato de ruta no disponible
/// en el destino. No reescribe rutas entre sintaxis de sistemas operativos; las omite y permite
/// que los defaults de la plataforma local entren en vigor.
fn sanitize_imported_settings(
    settings: &mut serde_json::Value,
    target: ConfigTarget,
) -> Vec<SkippedPlatformSetting> {
    let Some(settings) = settings.as_object_mut() else {
        return Vec::new();
    };

    let mut skipped = Vec::new();
    if let Some(path) = settings.get("home_dir").and_then(serde_json::Value::as_str) {
        if !path.is_empty() && !is_absolute_for_target(path, target) {
            settings.remove("home_dir");
            skipped.push(SkippedPlatformSetting::HomePath);
        }
    }

    if let Some(paths) = settings
        .get_mut("task_space_recents")
        .and_then(serde_json::Value::as_array_mut)
    {
        let before = paths.len();
        paths.retain(|value| {
            value
                .as_str()
                .is_some_and(|path| is_absolute_for_target(path, target))
        });
        if paths.len() != before {
            skipped.push(SkippedPlatformSetting::RecentPaths);
        }
    }

    if target == ConfigTarget::Linux {
        for (key, warning) in [("autostart", SkippedPlatformSetting::Autostart)] {
            if settings
                .get(key)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                settings.insert(key.to_string(), serde_json::Value::Bool(false));
                skipped.push(warning);
            }
        }
        if settings
            .get("global_hotkey_enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            skipped.push(SkippedPlatformSetting::GlobalHotkey);
        }
        // Linux aún no registra el atajo global; nunca importar el default Windows como
        // activado o mostrar una preferencia que no puede ejecutarse.
        settings.insert(
            "global_hotkey_enabled".to_string(),
            serde_json::Value::Bool(false),
        );
        if settings
            .get("autostart_minimized")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            && !settings
                .get("autostart")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        {
            settings.insert(
                "autostart_minimized".to_string(),
                serde_json::Value::Bool(false),
            );
        }
    }

    skipped
}

fn is_absolute_for_target(path: &str, target: ConfigTarget) -> bool {
    if path.is_empty() || path.contains('\0') {
        return false;
    }
    match target {
        ConfigTarget::Linux => path.starts_with('/'),
        ConfigTarget::Windows => {
            let bytes = path.as_bytes();
            (bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'/' | b'\\'))
                || path.starts_with("\\\\")
                || path.starts_with("//")
        }
        ConfigTarget::Other => Path::new(path).is_absolute(),
    }
}

#[cfg(test)]
mod worker_tests {
    use super::*;

    #[test]
    fn export_config_se_ejecuta_en_worker_y_despierta_la_ui() {
        let config = tempfile::tempdir().unwrap();
        std::fs::write(config.path().join("settings.json"), "{}".as_bytes()).unwrap();
        let output = config.path().join("config.naygoconf");
        let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(1);
        let worker = PackIo::new(std::sync::Arc::new(move || {
            let _ = wake_tx.try_send(());
        }));

        worker
            .submit(PackJob::ExportConfig {
                config_dir: config.path().to_path_buf(),
                output: output.clone(),
            })
            .unwrap();
        wake_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("el worker avisa al terminar");

        assert!(output.is_file());
        assert!(matches!(
            worker.drain().as_slice(),
            [PackOutcome::ExportTopic(Ok(()))]
        ));
    }

    #[test]
    fn catalogos_de_usuario_se_cargan_en_worker_y_despiertan_la_ui() {
        let config = tempfile::tempdir().unwrap();
        let themes = config.path().join("themes");
        let icons = config.path().join("icons").join("custom-set");
        std::fs::create_dir_all(&themes).unwrap();
        std::fs::create_dir_all(&icons).unwrap();
        std::fs::write(
            themes.join("custom-theme.json"),
            include_str!("../../core/src/theme/builtin/dark-blue.json"),
        )
        .unwrap();
        let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(1);
        let worker = PackIo::new(std::sync::Arc::new(move || {
            let _ = wake_tx.try_send(());
        }));

        worker
            .submit(PackJob::LoadCatalogs {
                config_dir: config.path().to_path_buf(),
                data_dir: config.path().to_path_buf(),
                active_theme: "custom-theme".into(),
            })
            .unwrap();
        wake_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("el worker avisa al terminar");

        let outcomes = worker.drain();
        let [PackOutcome::CatalogsLoaded {
            result: Ok((themes, icons)),
        }] = outcomes.as_slice()
        else {
            panic!("se esperaba la carga completa de los dos catálogos");
        };
        assert!(themes
            .available()
            .iter()
            .any(|id| id.as_str() == "custom-theme"));
        assert!(icons.available().iter().any(|set| set.id == "custom-set"));
    }

    #[test]
    fn guardar_tema_en_worker_es_atomico_actualiza_catalogo_y_retira_nombre_anterior() {
        let config = tempfile::tempdir().unwrap();
        let themes = config.path().join("themes");
        std::fs::create_dir_all(&themes).unwrap();
        std::fs::write(themes.join("old-name.json"), "tema anterior").unwrap();
        std::fs::write(
            themes.join("new-name.json"),
            "tema previo que debe reemplazarse",
        )
        .unwrap();
        let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(1);
        let worker = PackIo::new(std::sync::Arc::new(move || {
            let _ = wake_tx.try_send(());
        }));

        worker
            .submit(PackJob::SaveTheme {
                config_dir: config.path().to_path_buf(),
                id: "new-name".into(),
                old_id: Some("old-name".into()),
                json: include_str!("../../core/src/theme/builtin/dark-blue.json").into(),
            })
            .unwrap();
        wake_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("el worker avisa al terminar");

        let outcomes = worker.drain();
        let [PackOutcome::SaveTheme(Ok((id, catalog)))] = outcomes.as_slice() else {
            panic!("se esperaba el tema guardado y su catálogo");
        };
        assert_eq!(id, "new-name");
        assert!(themes.join("new-name.json").is_file());
        assert_eq!(
            std::fs::read_to_string(themes.join("new-name.json")).unwrap(),
            include_str!("../../core/src/theme/builtin/dark-blue.json")
        );
        assert!(!themes.join("new-name.json.tmp").exists());
        assert!(!themes.join("old-name.json").exists());
        assert!(catalog
            .available()
            .iter()
            .any(|theme_id| theme_id.as_str() == "new-name"));
    }

    #[test]
    fn borrar_tema_en_worker_devuelve_catalogo_y_tolera_que_ya_no_exista() {
        let config = tempfile::tempdir().unwrap();
        let themes = config.path().join("themes");
        std::fs::create_dir_all(&themes).unwrap();
        std::fs::write(
            themes.join("custom-theme.json"),
            include_str!("../../core/src/theme/builtin/dark-blue.json"),
        )
        .unwrap();
        let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(2);
        let worker = PackIo::new(std::sync::Arc::new(move || {
            let _ = wake_tx.try_send(());
        }));

        for expected_removed in [true, false] {
            worker
                .submit(PackJob::DeleteTheme {
                    config_dir: config.path().to_path_buf(),
                    id: "custom-theme".into(),
                })
                .unwrap();
            wake_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("el worker avisa al terminar");

            let outcomes = worker.drain();
            let [PackOutcome::DeleteTheme {
                id,
                result: Ok((removed, catalog)),
            }] = outcomes.as_slice()
            else {
                panic!("se esperaba el resultado del borrado y su catálogo");
            };
            assert_eq!(id, "custom-theme");
            assert_eq!(*removed, expected_removed);
            assert!(!themes.join("custom-theme.json").exists());
            assert!(!catalog
                .available()
                .iter()
                .any(|theme_id| theme_id.as_str() == "custom-theme"));
        }
    }
}

/// Mapea el nombre de una entrada del zip a (subcarpeta destino relativa, tipo). La subcarpeta
/// se deriva del nombre (puede tener un nivel intermedio como `icons/<id>`). Error si no se
/// reconoce.
fn classify(name: &str) -> Result<(String, ImportKind), String> {
    if let Some(rest) = name.strip_prefix("lang/") {
        if let Some(code) = rest.strip_suffix(".json") {
            return Ok(("lang".to_string(), ImportKind::Lang(code.to_string())));
        }
    }
    if let Some(rest) = name.strip_prefix("themes/") {
        if let Some(id) = rest.strip_suffix(".json") {
            return Ok(("themes".to_string(), ImportKind::Theme(id.to_string())));
        }
    }
    // Set de íconos de un tema: icons/<id>/<name>.png → carpeta icons/<id>; tipo Theme(id) para
    // que la UI recargue el catálogo de sets/temas tras importar.
    if let Some(rest) = name.strip_prefix("icons/") {
        if rest.to_ascii_lowercase().ends_with(".png") {
            if let Some((id, _file)) = rest.split_once('/') {
                if !id.is_empty() {
                    return Ok((format!("icons/{id}"), ImportKind::Theme(id.to_string())));
                }
            }
        }
    }
    if name == "settings.json" || name == "keybindings.json" {
        return Ok((String::new(), ImportKind::Config));
    }
    Err(format!("entrada no reconocida en el pack: {name}"))
}

/// Crea la subcarpeta destino y escribe los bytes de una entrada del pack en `config_dir/name`.
fn write_entry(config_dir: &Path, subdir: String, name: &str, bytes: &[u8]) -> Result<(), String> {
    let dest_dir = config_dir.join(&subdir);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("no se pudo crear {}: {e}", dest_dir.display()))?;
    let dest = config_dir.join(name);
    std::fs::write(&dest, bytes).map_err(|e| format!("no se pudo escribir {}: {e}", dest.display()))
}

/// ¿Los bytes empiezan con la firma PNG (\x89PNG\r\n\x1a\n)? Validación barata anti-basura.
fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_e_import_idioma_round_trip() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        // Crear lang/xx.json en el dir origen.
        std::fs::create_dir_all(src.path().join("lang")).unwrap();
        std::fs::write(src.path().join("lang/xx.json"), r#"{"app.loading":"…"}"#).unwrap();
        let zip_path = src.path().join("pack.zip");

        export_lang(src.path(), "xx", &zip_path).unwrap();
        assert!(zip_path.exists());

        let kind = import_zip(dst.path(), &zip_path).unwrap();
        assert_eq!(kind, ImportKind::Lang("xx".to_string()));
        assert!(dst.path().join("lang/xx.json").exists());
    }

    #[test]
    fn import_detecta_por_contenido_no_por_extension() {
        // El export por tópico usa extensiones propias (.naygolang/.naygotheme/.naygoconf), pero
        // el contenido sigue siendo un .zip y `import_zip` decide por el CONTENIDO, no por la
        // extensión. Un .naygolang (zip de idioma renombrado) debe detectarse como Lang.
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("lang")).unwrap();
        std::fs::write(src.path().join("lang/xx.json"), r#"{"app.loading":"…"}"#).unwrap();
        // Exportar con extensión de tópico (.naygolang) en vez de .zip.
        let pack = src.path().join("xx.naygolang");
        export_lang(src.path(), "xx", &pack).unwrap();
        assert!(pack.exists());

        // Importar el .naygolang: el backend ignora la extensión y detecta Lang por el contenido.
        let kind = import_zip(dst.path(), &pack).unwrap();
        assert_eq!(kind, ImportKind::Lang("xx".to_string()));
        assert!(dst.path().join("lang/xx.json").exists());
    }

    #[test]
    fn export_tema_inexistente_falla() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("t.zip");
        // dark-blue es embebido (no hay archivo) → debe fallar limpio.
        let err = export_theme(dir.path(), "dark-blue", &zip_path).unwrap_err();
        assert!(err.contains("embebido"));
        assert!(!zip_path.exists());
    }

    #[test]
    fn import_zip_invalido_falla() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.zip");
        std::fs::write(&bad, b"esto no es un zip").unwrap();
        assert!(import_zip(dir.path(), &bad).is_err());
    }

    /// Un PNG mínimo válido (firma + IHDR + IEND), suficiente para la validación por firma.
    fn png_minimo() -> Vec<u8> {
        let mut v = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(b"resto-cualquiera");
        v
    }

    #[test]
    fn export_import_tema_con_iconos_round_trip() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        // Tema con su set de íconos propio.
        std::fs::create_dir_all(src.path().join("themes")).unwrap();
        std::fs::write(src.path().join("themes/mio.json"), r#"{"name":"mio"}"#).unwrap();
        std::fs::create_dir_all(src.path().join("icons/mio")).unwrap();
        std::fs::write(src.path().join("icons/mio/folder.png"), png_minimo()).unwrap();
        let zip_path = src.path().join("tema.zip");

        export_theme(src.path(), "mio", &zip_path).unwrap();
        let kind = import_zip(dst.path(), &zip_path).unwrap();
        assert_eq!(kind, ImportKind::Theme("mio".to_string()));
        assert!(dst.path().join("themes/mio.json").exists());
        assert!(
            dst.path().join("icons/mio/folder.png").exists(),
            "el PNG del set de íconos del tema se importó"
        );
    }

    #[test]
    fn importa_iconos_en_data_y_exporta_con_fallback_legacy() {
        let config = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(config.path().join("themes")).unwrap();
        std::fs::write(config.path().join("themes/mio.json"), r#"{"name":"mio"}"#).unwrap();
        std::fs::create_dir_all(config.path().join("icons/mio")).unwrap();
        std::fs::write(config.path().join("icons/mio/legacy.png"), png_minimo()).unwrap();

        let pack = config.path().join("tema.zip");
        let file = std::fs::File::create(&pack).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("icons/mio/fresh.png", opts()).unwrap();
        zip.write_all(&png_minimo()).unwrap();
        zip.finish().unwrap();

        assert_eq!(
            import_zip_with_data(config.path(), data.path(), &pack).unwrap(),
            ImportKind::Theme("mio".into())
        );
        assert!(data.path().join("icons/mio/fresh.png").is_file());
        assert!(!config.path().join("icons/mio/fresh.png").exists());

        let exported = config.path().join("export.naygotheme");
        export_theme_with_data(config.path(), data.path(), "mio", &exported).unwrap();
        let mut archive = zip::ZipArchive::new(std::fs::File::open(exported).unwrap()).unwrap();
        let mut names = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            [
                "icons/mio/fresh.png",
                "icons/mio/legacy.png",
                "themes/mio.json"
            ]
        );
    }

    #[test]
    fn import_png_no_valido_falla() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        // Empaquetar a mano un icons/x/evil.png que NO es PNG.
        let zip_path = src.path().join("p.zip");
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file("icons/x/evil.png", opts()).unwrap();
        zw.write_all(b"no es png").unwrap();
        zw.finish().unwrap();

        assert!(import_zip(dst.path(), &zip_path).is_err());
        assert!(!dst.path().join("icons/x/evil.png").exists());
    }

    #[test]
    fn import_json_invalido_en_pack_falla() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        // Empaquetar a mano un themes/x.json que NO es JSON.
        let zip_path = src.path().join("p.zip");
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file("themes/x.json", opts()).unwrap();
        zw.write_all(b"no json").unwrap();
        zw.finish().unwrap();

        assert!(import_zip(dst.path(), &zip_path).is_err());
        // No debe haber escrito el archivo inválido.
        assert!(!dst.path().join("themes/x.json").exists());
    }

    #[test]
    fn import_config_linux_omite_rutas_windows_y_capacidades_no_disponibles() {
        let mut settings = serde_json::json!({
            "home_dir": "D:\\Trabajo",
            "task_space_recents": ["D:\\Trabajo\\Fotos", "/mnt/share", "relativa"],
            "autostart": true,
            "autostart_minimized": true,
            "global_hotkey_enabled": true,
            "global_hotkey": { "key": "Z", "ctrl": true, "alt": true }
        });

        let skipped = sanitize_imported_settings(&mut settings, ConfigTarget::Linux);

        assert_eq!(skipped.len(), 4);
        assert!(skipped.contains(&SkippedPlatformSetting::HomePath));
        assert!(skipped.contains(&SkippedPlatformSetting::RecentPaths));
        assert!(skipped.contains(&SkippedPlatformSetting::Autostart));
        assert!(skipped.contains(&SkippedPlatformSetting::GlobalHotkey));
        assert!(settings.get("home_dir").is_none());
        assert_eq!(
            settings["task_space_recents"],
            serde_json::json!(["/mnt/share"])
        );
        assert_eq!(settings["autostart"], false);
        assert_eq!(settings["autostart_minimized"], false);
        assert_eq!(settings["global_hotkey_enabled"], false);
        // Las combinaciones de teclado normales son portables y no se reescriben.
        assert_eq!(
            settings["global_hotkey"],
            serde_json::json!({"key":"Z","ctrl":true,"alt":true})
        );
    }

    #[test]
    fn import_config_conserva_rutas_nativas_windows_y_no_toca_ajustes_portables() {
        let mut settings = serde_json::json!({
            "home_dir": "D:\\Trabajo",
            "task_space_recents": ["D:\\Trabajo\\Fotos", "\\\\server\\share"],
            "autostart": true,
            "global_hotkey_enabled": true,
            "global_hotkey": { "key": "Z", "ctrl": true, "alt": true }
        });

        let skipped = sanitize_imported_settings(&mut settings, ConfigTarget::Windows);

        assert!(skipped.is_empty());
        assert_eq!(settings["home_dir"], "D:\\Trabajo");
        assert_eq!(settings["task_space_recents"].as_array().unwrap().len(), 2);
        assert_eq!(settings["autostart"], true);
        assert_eq!(settings["global_hotkey_enabled"], true);
    }

    #[test]
    fn import_config_linux_conserva_rutas_posix_y_no_avisa_si_no_omite_ajustes() {
        let mut settings = serde_json::json!({
            "home_dir": "/home/nicolas/Trabajo",
            "task_space_recents": ["/home/nicolas/Fotos"],
            "autostart": false,
            "global_hotkey_enabled": false
        });

        let skipped = sanitize_imported_settings(&mut settings, ConfigTarget::Linux);

        assert!(skipped.is_empty());
        assert_eq!(settings["home_dir"], "/home/nicolas/Trabajo");
        assert_eq!(settings["task_space_recents"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn import_pack_de_config_sanitiza_antes_de_escribir_y_devuelve_el_detalle() {
        let src = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let zip_path = src.path().join("config.naygoconf");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("settings.json", opts()).unwrap();
        zip.write_all(
            br#"{"home_dir":"C:\\Users\\nicolas","task_space_recents":["C:\\Fotos","/mnt/photos"],"autostart":true,"global_hotkey_enabled":true}"#,
        )
        .unwrap();
        zip.start_file("keybindings.json", opts()).unwrap();
        zip.write_all(br#"{"bindings":[]}"#).unwrap();
        zip.finish().unwrap();

        let report =
            import_zip_with_data_report(config.path(), data.path(), &zip_path, ConfigTarget::Linux)
                .unwrap();

        assert_eq!(report.kind, ImportKind::Config);
        assert_eq!(
            report.skipped_platform_settings,
            vec![
                SkippedPlatformSetting::HomePath,
                SkippedPlatformSetting::RecentPaths,
                SkippedPlatformSetting::Autostart,
                SkippedPlatformSetting::GlobalHotkey,
            ]
        );
        let imported: serde_json::Value =
            serde_json::from_slice(&std::fs::read(config.path().join("settings.json")).unwrap())
                .unwrap();
        assert!(imported.get("home_dir").is_none());
        assert_eq!(
            imported["task_space_recents"],
            serde_json::json!(["/mnt/photos"])
        );
        assert_eq!(imported["autostart"], false);
        assert_eq!(imported["global_hotkey_enabled"], false);
        // Atajos configurables no se modifican al filtrar ajustes de plataforma.
        assert_eq!(
            std::fs::read(config.path().join("keybindings.json")).unwrap(),
            br#"{"bindings":[]}"#
        );
    }
}
