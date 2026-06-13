// Naygo — puente entre el estado del panel (core) y el modelo de filas de Slint (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use naygo_core::workspace::FilePaneState;

/// Fila plana lista para pintar (espejo de `RowData` de Slint, pero sin depender de los
/// tipos generados → testeable en core puro). `controller` la convierte a `RowData`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlainRow {
    pub name: String,
    pub ext: String,
    pub size: String,
    pub modified: String,
    pub is_dir: bool,
    pub selected: bool,
    pub focused: bool,
}

/// Construye las filas a pintar desde el estado del panel: usa los índices de vista
/// CACHEADOS del core (filtrados+ordenados), y marca selección/foco por POSICIÓN DE
/// VISTA (consistente con `FilePaneState.selected`/`focused`). No clona las entries
/// completas: lee por índice.
pub fn rows_from_view(f: &FilePaneState) -> Vec<PlainRow> {
    let view = f.view_indices();
    view.iter()
        .enumerate()
        .filter_map(|(pos, &real)| {
            let e = f.entries.get(real)?;
            let ext = e
                .path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let size = match e.size {
                Some(b) => naygo_core::format::human_size(b),
                None => String::new(),
            };
            Some(PlainRow {
                name: e.name.clone(),
                ext,
                size,
                modified: fmt_time(e.modified),
                is_dir: e.kind == naygo_core::fs_model::EntryKind::Directory,
                selected: f.selected.contains(&pos),
                focused: f.focused == Some(pos),
            })
        })
        .collect()
}

/// Formato provisional de fecha (epoch en segundos), igual que la capa egui actual; el
/// formato bonito es ortogonal a la migración.
fn fmt_time(t: Option<std::time::SystemTime>) -> String {
    use std::time::UNIX_EPOCH;
    match t.and_then(|t| t.duration_since(UNIX_EPOCH).ok()) {
        Some(d) => format!("{}", d.as_secs()),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::fs_model::{Entry, EntryKind};
    use naygo_core::workspace::FilePaneState;
    use std::path::PathBuf;

    fn mk(name: &str, dir: bool, size: Option<u64>) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from(format!("C:/x/{name}")),
            kind: if dir {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size,
            modified: None,
            created: None,
            hidden: false,
        }
    }

    #[test]
    fn rows_reflejan_vista_seleccion_y_foco() {
        let mut f = FilePaneState::new(PathBuf::from("C:/x"));
        f.entries = vec![mk("a.txt", false, Some(1024)), mk("dir", true, None)];
        // Ordenado por core: dirs_first → "dir" primero, "a.txt" después.
        f.select_single(0); // selecciona la 1ª de la vista
        let rows = rows_from_view(&f);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.name == "dir" && r.is_dir));
        assert!(rows
            .iter()
            .any(|r| r.name == "a.txt" && !r.is_dir && !r.size.is_empty()));
        assert_eq!(rows.iter().filter(|r| r.selected).count(), 1);
        assert_eq!(rows.iter().filter(|r| r.focused).count(), 1);
    }

    #[test]
    fn vista_vacia_da_modelo_vacio() {
        let f = FilePaneState::new(PathBuf::from("C:/x"));
        assert!(rows_from_view(&f).is_empty());
    }
}
