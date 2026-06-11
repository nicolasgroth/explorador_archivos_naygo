// Naygo — carpetas recientes (MRU global, persistente como lista de rutas).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Historial global de carpetas visitadas, en orden MRU (la más reciente primero),
//! con tope y sin duplicados. Se persiste como JSON (solo rutas, nada de contenido).
//! Lo consumen: el menú del botón atrás (hoy), y el autocompletado del path + la
//! sección Recientes del panel Favoritos (fase siguiente).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Tope de carpetas recordadas.
const MAX_RECENTS: usize = 30;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecentDirs {
    /// La más reciente PRIMERO.
    dirs: Vec<PathBuf>,
}

impl RecentDirs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra una visita: `dir` pasa al frente (sin duplicados, tope MAX_RECENTS).
    pub fn push(&mut self, dir: PathBuf) {
        self.dirs.retain(|d| d != &dir);
        self.dirs.insert(0, dir);
        self.dirs.truncate(MAX_RECENTS);
    }

    /// Las recientes, la más nueva primero.
    pub fn list(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// Quita las carpetas que ya no existen (se llama antes de MOSTRAR la lista,
    /// nunca en el hilo caliente: son `exists()` de metadata local).
    pub fn remove_missing(&mut self) {
        self.dirs.retain(|d| d.exists());
    }

    /// Serializa a JSON (pretty: el archivo es diminuto y queda inspeccionable).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Carga tolerante: JSON corrupto o ausente → lista vacía.
    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).unwrap_or_default()
    }
}

/// Ruta del archivo de recientes dentro de la carpeta de configuración.
pub fn recents_path(config_dir: &Path) -> PathBuf {
    config_dir.join("recents.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn push_es_mru_sin_duplicados() {
        let mut r = RecentDirs::new();
        r.push(p("D:/a"));
        r.push(p("D:/b"));
        r.push(p("D:/a")); // re-visita: sube al frente, sin duplicar
        assert_eq!(r.list(), &[p("D:/a"), p("D:/b")]);
    }

    #[test]
    fn respeta_el_tope() {
        let mut r = RecentDirs::new();
        for i in 0..40 {
            r.push(p(&format!("D:/d{i}")));
        }
        assert_eq!(r.list().len(), MAX_RECENTS);
        assert_eq!(r.list()[0], p("D:/d39"));
    }

    #[test]
    fn json_round_trip_y_carga_tolerante() {
        let mut r = RecentDirs::new();
        r.push(p("D:/uno"));
        r.push(p("D:/dos"));
        let back = RecentDirs::from_json(&r.to_json());
        assert_eq!(back.list(), r.list());
        assert!(RecentDirs::from_json("{corrupto").list().is_empty());
        assert!(RecentDirs::from_json("").list().is_empty());
    }

    #[test]
    fn remove_missing_filtra_inexistentes() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().to_path_buf();
        let mut r = RecentDirs::new();
        r.push(p("D:/no/existe/jamas"));
        r.push(real.clone());
        r.remove_missing();
        assert_eq!(r.list(), &[real]);
    }
}
