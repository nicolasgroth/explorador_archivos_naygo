// Naygo — favoritos: carpetas ancladas por el usuario (puro, persistente en JSON).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modelo PURO de las carpetas favoritas: una lista ordenada (el orden define los
//! atajos `Ctrl+1..9`) de rutas con etiqueta. Sin egui ni Windows. Lo consumen la
//! path-bar (estrella ☆/★), el panel Favoritos, la sección anclada del árbol y los
//! atajos de salto. Persistencia en `<config>/favorites.json` (carga tolerante:
//! corrupto o ausente → lista vacía, nunca cae la app).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Una carpeta favorita: ruta + etiqueta visible (hoy el nombre de la carpeta;
/// editable a futuro sin migración, por eso se persiste aparte de la ruta).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Favorite {
    pub path: PathBuf,
    pub label: String,
}

/// La colección de favoritos, en el orden que ve el usuario (1º = `Ctrl+1`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Favorites {
    items: Vec<Favorite>,
}

/// Etiqueta por defecto de una ruta: el nombre de la carpeta, o la ruta completa
/// para raíces de unidad ("D:\" no tiene `file_name`).
fn default_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl Favorites {
    pub fn new() -> Self {
        Self::default()
    }

    /// ¿La ruta ya es favorita?
    pub fn contains(&self, path: &Path) -> bool {
        self.items.iter().any(|f| f.path == path)
    }

    /// Agrega la ruta si no estaba (al final) o la quita si ya estaba.
    /// Devuelve `true` si quedó agregada, `false` si quedó quitada.
    pub fn toggle(&mut self, path: &Path) -> bool {
        if self.contains(path) {
            self.remove(path);
            false
        } else {
            self.items.push(Favorite {
                path: path.to_path_buf(),
                label: default_label(path),
            });
            true
        }
    }

    /// Quita la ruta (no-op si no estaba).
    pub fn remove(&mut self, path: &Path) {
        self.items.retain(|f| f.path != path);
    }

    /// Los favoritos, en orden de usuario (el índice 0 corresponde a `Ctrl+1`).
    pub fn list(&self) -> &[Favorite] {
        &self.items
    }

    /// Serializa a JSON (pretty: archivo diminuto e inspeccionable, como recents).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Carga tolerante: JSON corrupto o ausente → lista vacía.
    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).unwrap_or_default()
    }
}

/// Ruta del archivo de favoritos dentro de la carpeta de configuración.
pub fn favorites_path(config_dir: &Path) -> PathBuf {
    config_dir.join("favorites.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn toggle_agrega_y_quita() {
        let mut f = Favorites::new();
        assert!(f.toggle(&p("D:\\Empresas\\ISGroth")), "1º toggle agrega");
        assert!(f.contains(&p("D:\\Empresas\\ISGroth")));
        assert_eq!(f.list().len(), 1);
        assert_eq!(f.list()[0].label, "ISGroth");
        assert!(!f.toggle(&p("D:\\Empresas\\ISGroth")), "2º toggle quita");
        assert!(!f.contains(&p("D:\\Empresas\\ISGroth")));
        assert!(f.list().is_empty());
    }

    #[test]
    fn raiz_de_unidad_usa_la_ruta_como_etiqueta() {
        let mut f = Favorites::new();
        f.toggle(&p("D:\\"));
        assert_eq!(f.list()[0].label, "D:\\");
    }

    #[test]
    fn el_orden_de_insercion_se_conserva() {
        let mut f = Favorites::new();
        f.toggle(&p("C:\\uno"));
        f.toggle(&p("C:\\dos"));
        f.toggle(&p("C:\\tres"));
        let labels: Vec<&str> = f.list().iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["uno", "dos", "tres"]);
    }

    #[test]
    fn remove_quita_solo_la_ruta_pedida() {
        let mut f = Favorites::new();
        f.toggle(&p("C:\\uno"));
        f.toggle(&p("C:\\dos"));
        f.remove(&p("C:\\uno"));
        assert!(!f.contains(&p("C:\\uno")));
        assert!(f.contains(&p("C:\\dos")));
        // Quitar algo que no está es un no-op.
        f.remove(&p("C:\\no_existe"));
        assert_eq!(f.list().len(), 1);
    }

    #[test]
    fn json_round_trip() {
        let mut f = Favorites::new();
        f.toggle(&p("D:\\Empresas"));
        f.toggle(&p("C:\\"));
        let back = Favorites::from_json(&f.to_json());
        assert_eq!(back.list(), f.list());
    }

    #[test]
    fn carga_corrupta_o_vacia_cae_a_default() {
        assert!(Favorites::from_json("{corrupto").list().is_empty());
        assert!(Favorites::from_json("").list().is_empty());
        assert!(Favorites::from_json("[1,2,3]").list().is_empty());
    }

    #[test]
    fn favorites_path_apunta_al_json() {
        let dir = p("C:\\config");
        assert_eq!(favorites_path(&dir), p("C:\\config\\favorites.json"));
    }
}
