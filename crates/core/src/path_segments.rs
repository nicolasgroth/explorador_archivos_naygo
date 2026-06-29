// Naygo — soporte puro de la path-bar: breadcrumbs y autocompletado.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Funciones PURAS que alimentan la path-bar interactiva (sin egui ni I/O):
//! - `split_segments`: ruta → segmentos clicables con su ruta acumulada.
//! - `split_edit_buffer`: texto en edición → (prefijo de carpeta padre, último
//!   segmento tecleado), para saber QUÉ carpeta listar y QUÉ filtrar.
//! - `filter_candidates`: filtra nombres por prefijo case-insensitive (el
//!   autocompletado del modo edición).

use std::path::{Component, Path, PathBuf};

/// Divide una ruta en segmentos `(etiqueta, ruta acumulada)` para pintar
/// breadcrumbs clicables. La raíz de unidad se entrega como UN segmento
/// ("D:\" → `("D:\", "D:\")`); un prefijo UNC (`\\server\share`) también.
/// `D:\Empresas\ISGroth` → `[("D:\", D:\), ("Empresas", D:\Empresas),
/// ("ISGroth", D:\Empresas\ISGroth)]`.
pub fn split_segments(path: &Path) -> Vec<(String, PathBuf)> {
    let mut segments = Vec::new();
    let mut acc = PathBuf::new();
    for comp in path.components() {
        match comp {
            // El prefijo solo ("D:" sin barra) es una ruta RELATIVA al cwd de esa
            // unidad en Windows: no se emite como segmento; se espera el RootDir
            // siguiente para emitir "D:\" (ruta absoluta segura de navegar).
            Component::Prefix(_) => acc.push(comp.as_os_str()),
            Component::RootDir => {
                acc.push(std::path::MAIN_SEPARATOR_STR);
                segments.push((acc.display().to_string(), acc.clone()));
            }
            Component::Normal(name) => {
                acc.push(name);
                segments.push((name.to_string_lossy().into_owned(), acc.clone()));
            }
            // `.` y `..` no aparecen en rutas canónicas de los paneles; si llegaran,
            // se acumulan sin emitir segmento (no hay carpeta útil que clicar).
            Component::CurDir | Component::ParentDir => acc.push(comp.as_os_str()),
        }
    }
    segments
}

/// Divide el texto del editor de ruta en `(prefijo padre, último segmento)`.
/// El prefijo padre INCLUYE el separador final, listo para concatenar un nombre
/// completado. Si no hay separador todavía, el padre es vacío (sin candidatos).
/// `"D:\Emp"` → `("D:\", "Emp")`; `"D:\Empresas\"` → `("D:\Empresas\", "")`.
pub fn split_edit_buffer(buffer: &str) -> (String, String) {
    match buffer.rfind(['\\', '/']) {
        Some(pos) => (buffer[..=pos].to_string(), buffer[pos + 1..].to_string()),
        None => (String::new(), buffer.to_string()),
    }
}

/// Filtra `names` por prefijo case-insensitive, conservando el orden de entrada,
/// hasta `max` resultados. Prefijo vacío → los primeros `max` nombres.
pub fn filter_candidates(names: &[String], prefix: &str, max: usize) -> Vec<String> {
    let needle = prefix.to_lowercase();
    names
        .iter()
        .filter(|n| n.to_lowercase().starts_with(&needle))
        .take(max)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_ruta_profunda() {
        let segs = split_segments(Path::new("D:\\Empresas\\ISGroth\\naygo"));
        let labels: Vec<&str> = segs.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["D:\\", "Empresas", "ISGroth", "naygo"]);
        assert_eq!(segs[0].1, PathBuf::from("D:\\"));
        assert_eq!(segs[1].1, PathBuf::from("D:\\Empresas"));
        assert_eq!(segs[3].1, PathBuf::from("D:\\Empresas\\ISGroth\\naygo"));
    }

    #[test]
    fn split_raiz_de_unidad_es_un_solo_segmento() {
        let segs = split_segments(Path::new("D:\\"));
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].0, "D:\\");
        assert_eq!(segs[0].1, PathBuf::from("D:\\"));
    }

    #[test]
    fn split_ruta_vacia_no_emite_segmentos() {
        assert!(split_segments(Path::new("")).is_empty());
    }

    #[test]
    fn split_unc_agrupa_el_prefijo() {
        let segs = split_segments(Path::new("\\\\server\\share\\docs"));
        // El prefijo UNC completo (\\server\share) + la barra raíz es el primer
        // segmento; "docs" el segundo.
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1].0, "docs");
        assert_eq!(segs[1].1, PathBuf::from("\\\\server\\share\\docs"));
    }

    #[test]
    fn split_edit_buffer_separa_padre_y_prefijo() {
        assert_eq!(
            split_edit_buffer("D:\\Emp"),
            ("D:\\".to_string(), "Emp".to_string())
        );
        assert_eq!(
            split_edit_buffer("D:\\Empresas\\"),
            ("D:\\Empresas\\".to_string(), "".to_string())
        );
        // Acepta también la barra normal (el usuario puede teclearla).
        assert_eq!(
            split_edit_buffer("D:/Empresas/IS"),
            ("D:/Empresas/".to_string(), "IS".to_string())
        );
        // Sin separador no hay padre que listar.
        assert_eq!(split_edit_buffer("D:"), ("".to_string(), "D:".to_string()));
        assert_eq!(split_edit_buffer(""), ("".to_string(), "".to_string()));
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn filter_es_case_insensitive_y_por_prefijo() {
        let ns = names(&["Empresas", "escritorio", "Backup", "EMPLEADOS"]);
        assert_eq!(filter_candidates(&ns, "emp", 12), ["Empresas", "EMPLEADOS"]);
        assert_eq!(filter_candidates(&ns, "ESC", 12), ["escritorio"]);
        assert!(filter_candidates(&ns, "zz", 12).is_empty());
    }

    #[test]
    fn filter_prefijo_vacio_devuelve_todo_hasta_max() {
        let ns = names(&["a", "b", "c", "d"]);
        assert_eq!(filter_candidates(&ns, "", 12).len(), 4);
        assert_eq!(filter_candidates(&ns, "", 2), ["a", "b"]);
    }

    #[test]
    fn filter_respeta_el_tope_con_prefijo() {
        let ns = names(&["ab1", "ab2", "ab3"]);
        assert_eq!(filter_candidates(&ns, "ab", 2), ["ab1", "ab2"]);
    }
}
