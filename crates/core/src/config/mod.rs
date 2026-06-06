// Naygo — persistencia portable del workspace/plantillas/ajustes (JSON).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Carga y guarda tres archivos JSON independientes junto al ejecutable
//! (portable). Tolerante: un archivo ausente, corrupto o de versión incompatible
//! NO crashea — se cae al default y se loguea. Cada archivo es independiente.

use crate::workspace::template::TemplateStore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Versión del formato de los archivos de config; permite migrar/descartar.
const CONFIG_VERSION: u32 = 1;

/// Dónde se ancla la barra de íconos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarPosition {
    Top,
    Side,
}

/// Qué set de íconos usa la app. Flat es el default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconSet {
    /// Multicolor plano (default).
    Flat,
    /// Estilo Fluent (Microsoft).
    Fluent,
    /// Monocromo temable (Lucide/Tabler).
    Mono,
}

/// Ajustes de la app (settings.json).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    pub bar_position: BarPosition,
    /// Botones de la barra solo con ícono (sin texto).
    pub icon_only: bool,
    /// Set de íconos activo.
    pub icon_set: IconSet,
    /// Mostrar la fila virtual ".." al tope del panel de archivos.
    pub show_parent_entry: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: CONFIG_VERSION,
            bar_position: BarPosition::Top,
            icon_only: true,
            icon_set: IconSet::Flat,
            show_parent_entry: true,
        }
    }
}

/// Estado persistible del workspace (workspace.json).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspacePersist {
    pub version: u32,
    /// La disposición (árbol de splits con PaneId).
    pub layout: crate::workspace::layout::SerializableDockLayout,
    /// El id del panel activo.
    pub active: Option<crate::workspace::PaneId>,
    /// Estado persistible de cada panel Files, indexado por PaneId.
    pub files: Vec<(
        crate::workspace::PaneId,
        crate::workspace::file_pane::FilePanePersist,
    )>,
    /// Tipo de cada panel del layout (para reconstruir Tree/Inspector también).
    pub purposes: Vec<(crate::workspace::PaneId, crate::workspace::PanePurpose)>,
}

/// Lee un archivo JSON y lo deserializa, devolviendo `None` si no existe o falla.
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<T>(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("config ilegible en {}: {e}", path.display());
            None
        }
    }
}

/// Escribe un valor como JSON (pretty). Loguea y traga el error (nunca crashea).
fn write_json<T: Serialize>(path: &Path, value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => {
            if let Err(e) = std::fs::write(path, text) {
                tracing::warn!("no se pudo guardar {}: {e}", path.display());
            }
        }
        Err(e) => tracing::warn!("no se pudo serializar {}: {e}", path.display()),
    }
}

/// Carga settings; si falta/corrupto/versión incompatible → default.
pub fn load_settings(dir: &Path) -> Settings {
    match read_json::<Settings>(&dir.join("settings.json")) {
        Some(s) if s.version == CONFIG_VERSION => s,
        Some(_) => {
            tracing::warn!("settings.json de versión incompatible; usando default");
            Settings::default()
        }
        None => Settings::default(),
    }
}

/// Guarda settings.
pub fn save_settings(dir: &Path, s: &Settings) {
    write_json(&dir.join("settings.json"), s);
}

/// Carga el store de plantillas; si falta/corrupto → vacío.
pub fn load_templates(dir: &Path) -> TemplateStore {
    read_json::<TemplateStore>(&dir.join("templates.json")).unwrap_or_default()
}

/// Guarda el store de plantillas.
pub fn save_templates(dir: &Path, store: &TemplateStore) {
    write_json(&dir.join("templates.json"), store);
}

/// Carga el workspace persistido; `None` si falta/corrupto/versión incompatible
/// (el llamador cae a la plantilla default).
pub fn load_workspace(dir: &Path) -> Option<WorkspacePersist> {
    match read_json::<WorkspacePersist>(&dir.join("workspace.json")) {
        Some(w) if w.version == CONFIG_VERSION => Some(w),
        Some(_) => {
            tracing::warn!("workspace.json de versión incompatible; ignorando");
            None
        }
        None => None,
    }
}

/// Guarda el workspace persistido.
pub fn save_workspace(dir: &Path, w: &WorkspacePersist) {
    write_json(&dir.join("workspace.json"), w);
}

/// Directorio de config portable: junto al ejecutable, o el cwd como fallback.
pub fn portable_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings {
            version: CONFIG_VERSION,
            bar_position: BarPosition::Side,
            icon_only: false,
            icon_set: IconSet::Mono,
            show_parent_entry: false,
        };
        save_settings(dir.path(), &s);
        assert_eq!(load_settings(dir.path()), s);
    }

    #[test]
    fn settings_default_tiene_iconos_flat_y_fila_padre_on() {
        let s = Settings::default();
        assert_eq!(s.icon_set, IconSet::Flat);
        assert!(s.show_parent_entry);
    }

    #[test]
    fn settings_ausente_da_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn settings_corrupto_da_default_sin_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), b"{ no es json valido").unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn settings_version_incompatible_da_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":999,"bar_position":"Top","icon_only":true}"#,
        )
        .unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn templates_ausente_da_vacio() {
        let dir = tempfile::tempdir().unwrap();
        let store = load_templates(dir.path());
        assert!(store.user.is_empty());
        assert!(store.recents.is_empty());
    }

    #[test]
    fn workspace_ausente_da_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_workspace(dir.path()).is_none());
    }

    #[test]
    fn workspace_round_trip_conserva_layout_y_activo() {
        use crate::workspace::layout::SerializableDockLayout;
        use crate::workspace::PaneId;

        let dir = tempfile::tempdir().unwrap();
        let persist = WorkspacePersist {
            version: CONFIG_VERSION,
            layout: SerializableDockLayout::single(PaneId(3)),
            active: Some(PaneId(3)),
            files: Vec::new(),
            purposes: vec![(PaneId(3), crate::workspace::PanePurpose::Files)],
        };
        save_workspace(dir.path(), &persist);
        let loaded = load_workspace(dir.path()).expect("debe cargar");
        assert_eq!(loaded.version, CONFIG_VERSION);
        assert_eq!(loaded.active, Some(PaneId(3)));
        assert_eq!(loaded.layout.pane_ids(), vec![PaneId(3)]);
        assert_eq!(loaded.purposes.len(), 1);
    }
}
