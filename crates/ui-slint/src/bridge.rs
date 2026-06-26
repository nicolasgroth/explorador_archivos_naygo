// Naygo — puente entre el estado del panel (core) y el modelo de filas de Slint (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use naygo_core::favorites::{FavNode, Favorites};
use naygo_core::ops::undo::{self, UndoEntry};
use naygo_core::recent_dirs::RecentDirs;
use naygo_core::tree::{DirTree, NodeState, TreeNode};
use naygo_core::workspace::FilePaneState;

/// Fila plana lista para pintar (espejo de `RowData` de Slint, pero sin depender de los
/// tipos generados → testeable en core puro). `controller` la convierte a `RowData`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlainRow {
    pub name: String,
    /// Celdas PARALELAS a las columnas visibles (mismo índice/orden, 6C). La celda de Name lleva
    /// el nombre (la UI la pinta como ícono+nombre); el resto son Extension/Size/Modified/Created.
    /// Mantenerlas paralelas a `columns` hace trivial el índice en Slint (`cells[ci]`).
    pub cells: Vec<String>,
    pub is_dir: bool,
    pub selected: bool,
    pub focused: bool,
    /// El ítem está marcado como "cortado" (Ctrl+X): se pinta atenuado hasta pegar.
    pub cut: bool,
    /// El ítem apareció recién (watcher): se pinta resaltado unos segundos. Fase 5A.
    pub highlight: bool,
    /// Ícono de color por tipo, ya decodificado y cacheado (6A). Lo resuelve el `icon_of`
    /// que pasa el controlador (consulta el `IconCache` por la clave del entry).
    pub icon: slint::Image,
    /// Profundidad en la vista profunda (recursiva): 0 = nivel raíz, >0 sangra la fila.
    /// En vista normal/búsqueda es 0 (sin efecto).
    pub depth: u32,
}

/// Los tres flags de visibilidad (espejo runtime de `Settings`): qué clases de archivo
/// muestra el panel/árbol. El tipo vive en `core` (`naygo_core::filter::VisibilityFlags`)
/// porque `FilePaneState` los usa para filtrar la VISTA; aquí se reexporta para el árbol
/// (`pump_tree`) y la vista profunda, que filtran aparte con `is_visible`. Default = mostrar
/// todo. El panel normal YA no re-filtra: el core entrega la vista filtrada (ver
/// `rows_from_view`).
pub use naygo_core::filter::VisibilityFlags;

/// Valor de texto de una celda para un entry y una columna (6C). Name se pinta aparte;
/// aquí se cubren las demás. Size respeta `size_format`; las fechas, `date_format`+huso.
pub fn cell_value(
    e: &naygo_core::fs_model::Entry,
    kind: naygo_core::columns::ColumnKind,
    size_format: naygo_core::format::SizeFormat,
    date_format: naygo_core::format::DateFormat,
    tz_offset_secs: i64,
) -> String {
    use naygo_core::columns::ColumnKind::*;
    match kind {
        Name => e.name.clone(),
        Extension => e
            .path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string(),
        Size => match e.size {
            Some(b) => naygo_core::format::format_size(b, size_format),
            None => String::new(),
        },
        Modified => fmt_time(e.modified, date_format, tz_offset_secs),
        Created => fmt_time(e.created, date_format, tz_offset_secs),
    }
}

/// Construye las filas a pintar desde el estado del panel: usa los índices de vista
/// CACHEADOS del core (filtrados+ordenados, INCLUIDA la visibilidad), y marca selección/foco
/// por POSICIÓN DE VISTA (consistente con `FilePaneState.selected`/`focused`). No clona las
/// entries completas: lee por índice. `is_cut` consulta si una ruta está marcada como cortada.
/// Las celdas siguen el orden de las columnas visibles del `TableState` (sin Name).
///
/// IMPORTANTE: aquí NO se re-filtra por visibilidad. El core ya aplica `is_visible` dentro de
/// `compute_view_indices`, así que las filas que llegan corresponden 1:1 con las posiciones de
/// vista (`i == pos`): esconder ocultos no desalinea selección/foco/teclado.
#[allow(clippy::too_many_arguments)]
pub fn rows_from_view(
    f: &FilePaneState,
    is_cut: &dyn Fn(&std::path::Path) -> bool,
    is_fresh: &dyn Fn(&std::path::Path) -> bool,
    icon_of: &mut dyn FnMut(&naygo_core::fs_model::Entry) -> slint::Image,
    size_format: naygo_core::format::SizeFormat,
    date_format: naygo_core::format::DateFormat,
    tz_offset_secs: i64,
) -> Vec<PlainRow> {
    // Columnas visibles en orden (incluida Name): paralelas a `columns_info`. Común a las filas.
    let cell_kinds: Vec<naygo_core::columns::ColumnKind> =
        f.table.visible_columns().map(|c| c.kind).collect();
    let view = f.view_indices();
    view.iter()
        .enumerate()
        .filter_map(|(pos, &real)| {
            let e = f.entries.get(real)?;
            let cells = cell_kinds
                .iter()
                .map(|k| cell_value(e, *k, size_format, date_format, tz_offset_secs))
                .collect();
            Some(PlainRow {
                name: e.name.clone(),
                cells,
                is_dir: e.kind == naygo_core::fs_model::EntryKind::Directory,
                selected: f.selected.contains(&pos),
                focused: f.focused == Some(pos),
                cut: is_cut(&e.path),
                highlight: is_fresh(&e.path),
                icon: icon_of(e),
                depth: 0,
            })
        })
        .collect()
}

/// Datos de una columna visible para la UI (espejo de `ColumnVm` de Slint): qué es, su etiqueta
/// traducida, su ancho y si se alinea a la derecha (Size). El orden es el visual del TableState.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnInfo {
    pub kind: i32,
    pub label: String,
    pub width: f32,
    pub align_right: bool,
    /// Indicador de orden en esta columna: 0=ninguno, 1=ascendente (▲), 2=descendente (▼).
    pub sort_dir: i32,
    /// Esta columna tiene un filtro activo (se pinta un embudo en el header).
    pub has_filter: bool,
}

/// Mapea `ColumnKind` a un entero estable para Slint (coincide con el orden del enum).
pub fn column_kind_to_int(kind: naygo_core::columns::ColumnKind) -> i32 {
    use naygo_core::columns::ColumnKind::*;
    match kind {
        Name => 0,
        Extension => 1,
        Size => 2,
        Modified => 3,
        Created => 4,
    }
}

/// Inverso de `column_kind_to_int`. Desconocido → Name (no rompe).
pub fn column_kind_from_int(i: i32) -> naygo_core::columns::ColumnKind {
    use naygo_core::columns::ColumnKind::*;
    match i {
        1 => Extension,
        2 => Size,
        3 => Modified,
        4 => Created,
        _ => Name,
    }
}

/// Columnas visibles del panel con su etiqueta i18n y alineación. `label_of` traduce la clave.
pub fn columns_info(
    f: &FilePaneState,
    label_of: &dyn Fn(naygo_core::columns::ColumnKind) -> String,
) -> Vec<ColumnInfo> {
    use naygo_core::columns::ColumnKind;
    // Clave de orden activa del panel → para marcar la columna ordenada con ▲/▼.
    let sort_key = f.sort.key;
    let sort_asc = f.sort.ascending;
    f.table
        .visible_columns()
        .map(|c| ColumnInfo {
            kind: column_kind_to_int(c.kind),
            label: label_of(c.kind),
            width: c.width,
            align_right: c.kind == ColumnKind::Size,
            sort_dir: if naygo_core::columns::sort_key_of(c.kind) == sort_key {
                if sort_asc {
                    1
                } else {
                    2
                }
            } else {
                0
            },
            has_filter: f.table.filters.contains_key(&c.kind),
        })
        .collect()
}

/// Una extensión marcable del editor de filtro de tipos (espejo de `ExtRowVm`, F2).
#[derive(Clone, Debug, PartialEq)]
pub struct ExtRowInfo {
    pub ext: String,
    pub count: usize,
    pub checked: bool,
}

/// Instantánea del menú/editor de columna para la UI (espejo de `ColumnMenuVm`, F2).
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnMenuInfo {
    pub x: f32,
    pub y: f32,
    pub kind: i32,
    pub label: String,
    pub mode: i32,
    pub has_filter: bool,
    pub can_hide: bool,
    pub text_draft: String,
    pub text_case: bool,
    pub min_draft: String,
    pub max_draft: String,
    pub exts: Vec<ExtRowInfo>,
}

/// Una entrada del menú "Columnas…" (espejo de `ColumnToggleVm`): TODAS las columnas con su
/// estado de visibilidad. Name es `fixed` (no se puede ocultar).
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnToggle {
    pub kind: i32,
    pub label: String,
    pub visible: bool,
    pub fixed: bool,
}

/// Todas las columnas del panel (en su orden), para el menú de agregar/quitar.
pub fn column_toggles(
    f: &FilePaneState,
    label_of: &dyn Fn(naygo_core::columns::ColumnKind) -> String,
) -> Vec<ColumnToggle> {
    use naygo_core::columns::ColumnKind;
    f.table
        .columns
        .iter()
        .map(|c| ColumnToggle {
            kind: column_kind_to_int(c.kind),
            label: label_of(c.kind),
            visible: c.visible,
            fixed: c.kind == ColumnKind::Name,
        })
        .collect()
}

/// Formatea la fecha de modificación legible según el ajuste, ajustando al huso local
/// (`tz_offset_secs`). Cadena vacía si no hay fecha.
fn fmt_time(
    t: Option<std::time::SystemTime>,
    fmt: naygo_core::format::DateFormat,
    tz_offset_secs: i64,
) -> String {
    use std::time::UNIX_EPOCH;
    let local = t
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64 + tz_offset_secs);
    naygo_core::format::format_time(local, fmt)
}

// --- Inspector (propiedades del ítem enfocado del panel activo) ---

/// Datos planos del inspector (espejo de `InspectorVm` de Slint). `present = false`
/// cuando no hay nada enfocado (la UI muestra el placeholder).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InspectorInfo {
    pub present: bool,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub size: String,
    pub modified: String,
    pub created: String,
}

/// Construye la info del inspector desde el `FilePaneState` del panel Files activo.
/// Sin foco → `InspectorInfo::default()` (present = false). El texto de `kind` es una
/// CLAVE provisional (la traducción real llega con i18n en F6); aquí va literal.
pub fn inspector_info(
    f: Option<&FilePaneState>,
    date_format: naygo_core::format::DateFormat,
    tz_offset_secs: i64,
) -> InspectorInfo {
    let Some(e) = f.and_then(|f| f.focused_view_entry()) else {
        return InspectorInfo::default();
    };
    use naygo_core::fs_model::EntryKind;
    let kind = match e.kind {
        EntryKind::Directory => "Carpeta",
        EntryKind::File => "Archivo",
        EntryKind::Other => "Otro",
    };
    InspectorInfo {
        present: true,
        name: e.name.clone(),
        kind: kind.to_string(),
        path: e.path.display().to_string(),
        size: match e.size {
            Some(b) => naygo_core::format::human_size(b),
            None => String::new(),
        },
        modified: fmt_time(e.modified, date_format, tz_offset_secs),
        created: fmt_time(e.created, date_format, tz_offset_secs),
    }
}

// --- Favoritos y recientes ---

/// Fila de favorito/reciente (espejo de `NavRow` de Slint): etiqueta + ruta + ícono. Favoritos
/// y recientes son siempre carpetas, así que el ícono es el de carpeta del set activo (6B).
#[derive(Clone, Debug, PartialEq)]
pub struct NavRow {
    pub label: String,
    pub path: String,
    pub icon: slint::Image,
    /// Solo para la tira de unidades: `true` si la unidad es extraíble (USB), para
    /// ofrecer la expulsión segura. Favoritos/recientes siempre van en `false`.
    pub removable: bool,
}

/// Favoritos en orden de usuario (índice 0 = Ctrl+1). `folder_icon` es el ícono de carpeta
/// ya cacheado (igual para todas las filas: comparten el mismo buffer).
pub fn favorite_rows(favs: &Favorites, folder_icon: &slint::Image) -> Vec<NavRow> {
    favs.list_flat()
        .iter()
        .map(|f| NavRow {
            label: f.label.clone(),
            path: f.path.display().to_string(),
            icon: folder_icon.clone(),
            removable: false,
        })
        .collect()
}

// --- Árbol de favoritos (aplanado a lista con sangría, con grupos) ---

/// Fila del árbol de favoritos (espejo de `FavTreeRow` de Slint). El árbol se aplana en
/// preorden a una lista; `depth` da la sangría. Para GRUPOS: `is_group=true`, `name` es el
/// nombre del grupo, `group_id` es su ruta de índices serializada ("0/2/1") y `path` va vacío.
/// Para FAVORITOS: `is_group=false`, `name`/`path` son la etiqueta y la ruta, `group_id` vacío.
/// `expanded` solo aplica a grupos; `has_children` indica si el grupo tiene hijos (chevron).
/// `name_path` es la "ruta de nombres" del nodo (clave ESTABLE de expansión que sobrevive al
/// reordenamiento de índices, a diferencia del `group_id` numérico).
#[derive(Clone, Debug, PartialEq)]
pub struct FavTreeRow {
    pub depth: i32,
    pub is_group: bool,
    pub name: String,
    pub path: String,
    pub group_id: String,
    pub name_path: String,
    pub expanded: bool,
    pub has_children: bool,
    pub icon: slint::Image,
}

/// Serializa un `GroupId` (`[0, 2, 1]`) a la cadena estable "0/2/1" para cruzar la frontera
/// a Slint. Vacío para la raíz. El controlador lo parsea de vuelta re-leyendo el árbol actual.
pub fn group_id_to_str(id: &[usize]) -> String {
    id.iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// Parsea la cadena "0/2/1" de vuelta a un `GroupId`. Cadena vacía → `Vec` vacío (la raíz).
pub fn str_to_group_id(s: &str) -> Vec<usize> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split('/').filter_map(|p| p.parse().ok()).collect()
}

/// Recorre el subárbol en preorden y empuja una `FavTreeRow` por nodo VISIBLE. Un grupo
/// colapsado (`name_path` no en `expanded`) no desciende a sus hijos. `id` es la ruta de
/// índices acumulada; `name_path` la ruta de nombres acumulada (clave de expansión).
fn push_fav_node(
    nodes: &[FavNode],
    depth: i32,
    parent_id: &[usize],
    parent_name_path: &str,
    expanded: &dyn Fn(&str) -> bool,
    folder_icon: &slint::Image,
    out: &mut Vec<FavTreeRow>,
) {
    for (i, node) in nodes.iter().enumerate() {
        let mut id = parent_id.to_vec();
        id.push(i);
        match node {
            FavNode::Favorite { path, label } => {
                let name_path = if parent_name_path.is_empty() {
                    format!("\u{1}{label}") // prefijo \u{1}: separa hojas de grupos homónimos
                } else {
                    format!("{parent_name_path}/\u{1}{label}")
                };
                out.push(FavTreeRow {
                    depth,
                    is_group: false,
                    name: label.clone(),
                    path: path.display().to_string(),
                    group_id: String::new(),
                    name_path,
                    expanded: false,
                    has_children: false,
                    icon: folder_icon.clone(),
                });
            }
            FavNode::Group { name, children } => {
                let name_path = if parent_name_path.is_empty() {
                    name.clone()
                } else {
                    format!("{parent_name_path}/{name}")
                };
                let is_expanded = expanded(&name_path);
                out.push(FavTreeRow {
                    depth,
                    is_group: true,
                    name: name.clone(),
                    path: String::new(),
                    group_id: group_id_to_str(&id),
                    name_path: name_path.clone(),
                    expanded: is_expanded,
                    has_children: !children.is_empty(),
                    icon: folder_icon.clone(),
                });
                if is_expanded {
                    push_fav_node(
                        children,
                        depth + 1,
                        &id,
                        &name_path,
                        expanded,
                        folder_icon,
                        out,
                    );
                }
            }
        }
    }
}

/// Aplana el árbol de favoritos a filas con sangría, respetando los grupos colapsados.
/// `expanded(name_path)` decide si un grupo se expande (la UI mantiene ese estado). El ícono
/// de carpeta del set activo se reusa en todas las filas (comparten buffer).
pub fn fav_tree_rows(
    favs: &Favorites,
    expanded: &dyn Fn(&str) -> bool,
    folder_icon: &slint::Image,
) -> Vec<FavTreeRow> {
    let mut out = Vec::new();
    push_fav_node(favs.roots(), 0, &[], "", expanded, folder_icon, &mut out);
    out
}

/// Recientes (los más recientes primero, según el orden que provea `RecentDirs`). La
/// etiqueta es el nombre de la carpeta; la ruta completa va en `path` (tooltip/navegar).
pub fn recent_rows(recents: &RecentDirs, folder_icon: &slint::Image) -> Vec<NavRow> {
    recents
        .list()
        .iter()
        .map(|p| NavRow {
            label: p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string()),
            path: p.display().to_string(),
            icon: folder_icon.clone(),
            removable: false,
        })
        .collect()
}

// --- Historial (deshacer) ---

/// Fila del historial (espejo de `HistRow` de Slint): etiqueta + cuándo + cuántas
/// acciones + si AÚN es deshacible (validado contra el disco) + motivo si no lo es.
///
/// `undone` distingue el caso "ya lo deshiciste" (la acción se aplicó) del caso
/// "no es deshacible por su tipo / ya no aplica" (p. ej. un Delete, o el destino
/// cambió). Ambos dejan `undoable = false`, pero la UI los pinta distinto: el primero
/// como "Deshecho ✓" (en color de acento, confirmación de que se aplicó), el segundo
/// con el botón inerte y el motivo de tooltip.
#[derive(Clone, Debug, PartialEq)]
pub struct HistRow {
    pub id: u64,
    pub label: String,
    pub when: String,
    pub count: i32,
    pub undoable: bool,
    /// `true` si esta entrada YA se deshizo (acción aplicada). Mutuamente excluyente
    /// con `undoable` (una entrada deshecha no se vuelve a ofrecer).
    pub undone: bool,
    pub reason: String,
}

/// Convierte el historial de deshacer a filas, validando cada entrada contra el disco
/// (igual que el panel egui: deshabilita "Deshacer" y muestra el motivo si ya no aplica).
/// Las entradas ya deshechas no se ofrecen (undoable = false, `undone = true`).
pub fn history_rows(entries: &[UndoEntry]) -> Vec<HistRow> {
    entries
        .iter()
        .map(|e| {
            // Tres estados: deshecha (undone), deshacible (undoable), o trabada/no-deshacible
            // (ninguna de las dos, con motivo). `undone` tiene prioridad: si ya se aplicó, no
            // importa que el inverso "siga válido", no se vuelve a ofrecer.
            let (undoable, reason) = if e.undone {
                (false, String::new())
            } else {
                match undo::validate(&e.actions) {
                    Ok(()) => (true, String::new()),
                    Err(why) => (false, why),
                }
            };
            HistRow {
                id: e.id,
                label: e.label.clone(),
                when: format!("{}", e.when_epoch_secs),
                count: e.actions.len() as i32,
                undoable,
                undone: e.undone,
                reason,
            }
        })
        .collect()
}

// --- Árbol de directorios (aplanado a lista con sangría) ---

/// Fila del árbol (espejo de `TreeRow` de Slint). El árbol se aplana en preorden a una
/// lista; `depth` da la sangría (la UI pinta `depth * indent`). `has_children` indica si
/// mostrar el chevron ▶/▼; `expanded` su estado; `active` resalta la carpeta del panel
/// activo; `disk_percent` (-1 si no aplica) pinta la barrita de uso en las raíces.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow {
    pub depth: i32,
    pub name: String,
    pub path: String,
    pub expanded: bool,
    pub has_children: bool,
    pub is_drive: bool,
    pub active: bool,
    pub loading: bool,
    pub error: bool,
    pub disk_percent: i32,
    /// Texto de espacio para las raíces de disco ("120 GB / 500 GB · 76%"); vacío si no es
    /// disco o no hay dato (red caída / óptico vacío). Acompaña a la barrita de `disk_percent`.
    pub disk_detail: String,
    /// Ícono de color cacheado (6A): carpeta o disco según `is_drive`.
    pub icon: slint::Image,
}

/// Aplana el árbol visible (solo nodos expandidos descienden) a una lista con sangría.
/// `disk` mapea ruta de raíz → uso del disco (total/libre); las raíces sin dato van a `None`
/// (barrita en -1, texto vacío). `icon_of` resuelve el ícono de cada nodo desde el cache.
pub fn tree_rows(
    tree: &DirTree,
    disk: &dyn Fn(&std::path::Path) -> Option<naygo_core::disk::DiskUsage>,
    icon_of: &mut dyn FnMut(naygo_core::icon_kind::IconKey) -> slint::Image,
) -> Vec<TreeRow> {
    let mut out = Vec::new();
    let active = tree.active_path.as_deref();
    for root in &tree.roots {
        push_tree_node(root, 0, active, disk, icon_of, &mut out);
    }
    out
}

/// Texto de espacio de un disco: "120 GB / 500 GB · 76%". Vacío si total es 0 (desconocido).
fn disk_detail_text(usage: naygo_core::disk::DiskUsage) -> String {
    use naygo_core::format::{format_size, SizeFormat};
    if usage.total == 0 {
        return String::new();
    }
    format!(
        "{} libre / {} · {}%",
        format_size(usage.free, SizeFormat::Auto),
        format_size(usage.total, SizeFormat::Auto),
        usage.percent_used()
    )
}

fn push_tree_node(
    node: &TreeNode,
    depth: i32,
    active: Option<&std::path::Path>,
    disk: &dyn Fn(&std::path::Path) -> Option<naygo_core::disk::DiskUsage>,
    icon_of: &mut dyn FnMut(naygo_core::icon_kind::IconKey) -> slint::Image,
    out: &mut Vec<TreeRow>,
) {
    let is_drive = node.drive_kind.is_some();
    // Un nodo "tiene hijos" (muestra chevron) si es una raíz/carpeta que no se ha probado
    // vacía: Collapsed/Loading/Loaded muestran chevron; Empty/Error no.
    let has_children = !matches!(node.state, NodeState::Empty | NodeState::Error);
    out.push(TreeRow {
        depth,
        name: node.name.clone(),
        path: node.path.display().to_string(),
        expanded: node.expanded,
        has_children,
        is_drive,
        active: active == Some(node.path.as_path()),
        loading: node.state == NodeState::Loading,
        error: node.state == NodeState::Error,
        disk_percent: if is_drive {
            disk(&node.path)
                .map(|u| u.percent_used() as i32)
                .unwrap_or(-1)
        } else {
            -1
        },
        disk_detail: if is_drive {
            disk(&node.path).map(disk_detail_text).unwrap_or_default()
        } else {
            String::new()
        },
        icon: icon_of(if is_drive {
            naygo_core::icon_kind::IconKey::Drive(
                node.drive_kind
                    .unwrap_or(naygo_core::icon_kind::DriveKind::Unknown),
            )
        } else {
            naygo_core::icon_kind::IconKey::Folder
        }),
    });
    if node.expanded {
        if let Some(children) = node.children.as_ref() {
            for child in children {
                push_tree_node(child, depth + 1, active, disk, icon_of, out);
            }
        }
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
            system: false,
        }
    }

    #[test]
    fn rows_reflejan_vista_seleccion_y_foco() {
        let mut f = FilePaneState::new(PathBuf::from("C:/x"));
        f.entries = vec![mk("a.txt", false, Some(1024)), mk("dir", true, None)];
        // Ordenado por core: dirs_first → "dir" primero, "a.txt" después.
        f.select_single(0); // selecciona la 1ª de la vista
        let rows = rows_from_view(
            &f,
            &|_| false,
            &|_| false,
            &mut |_| slint::Image::default(),
            naygo_core::format::SizeFormat::Auto,
            naygo_core::format::DateFormat::IsoMinute,
            0,
        );
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.name == "dir" && r.is_dir));
        // "a.txt" tiene tamaño: alguna de sus celdas (la de Size) no está vacía.
        assert!(rows
            .iter()
            .any(|r| r.name == "a.txt" && !r.is_dir && r.cells.iter().any(|c| c.contains("KB"))));
        assert_eq!(rows.iter().filter(|r| r.selected).count(), 1);
        assert_eq!(rows.iter().filter(|r| r.focused).count(), 1);
        assert!(rows.iter().all(|r| !r.cut), "sin corte por defecto");
    }

    #[test]
    fn cut_marca_la_fila() {
        let mut f = FilePaneState::new(PathBuf::from("C:/x"));
        f.entries = vec![mk("a.txt", false, Some(1))];
        let rows = rows_from_view(
            &f,
            &|p| p.ends_with("a.txt"),
            &|_| false,
            &mut |_| slint::Image::default(),
            naygo_core::format::SizeFormat::Auto,
            naygo_core::format::DateFormat::IsoMinute,
            0,
        );
        assert!(rows[0].cut, "la fila cortada se marca");
    }

    #[test]
    fn vista_vacia_da_modelo_vacio() {
        let f = FilePaneState::new(PathBuf::from("C:/x"));
        assert!(rows_from_view(
            &f,
            &|_| false,
            &|_| false,
            &mut |_| slint::Image::default(),
            naygo_core::format::SizeFormat::Auto,
            naygo_core::format::DateFormat::IsoMinute,
            0,
        )
        .is_empty());
    }

    #[test]
    fn visibilidad_esconde_filas_sin_desalinear_seleccion() {
        // Entries: [oculto, visible, sistema, .dot]. El core ordena por nombre/dirs_first,
        // pero todos son archivos, así que el orden de vista es alfabético:
        // ".dot", "b_oculto", "c_sys", "d_visible".
        let mut f = FilePaneState::new(PathBuf::from("C:/x"));
        let mut oculto = mk("b_oculto.txt", false, Some(1));
        oculto.hidden = true;
        let mut sys = mk("c_sys.txt", false, Some(1));
        sys.system = true;
        let dot = mk(".dot", false, Some(1));
        let visible = mk("d_visible.txt", false, Some(1));
        f.entries = vec![oculto, sys, dot, visible];

        // El filtro de visibilidad vive en el CORE: se setea en el estado y queda dentro de la
        // vista. `rows_from_view` ya NO re-filtra; sirve 1:1 lo que el core entrega. Con todo
        // escondido, solo queda el visible.
        f.set_visibility(VisibilityFlags {
            show_hidden: false,
            show_system: false,
            hide_dotfiles: true,
        });
        let rows = rows_from_view(
            &f,
            &|_| false,
            &|_| false,
            &mut |_| slint::Image::default(),
            naygo_core::format::SizeFormat::Auto,
            naygo_core::format::DateFormat::IsoMinute,
            0,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "d_visible.txt");

        // Mostrar todo: las cuatro filas.
        f.set_visibility(VisibilityFlags::default());
        let rows = rows_from_view(
            &f,
            &|_| false,
            &|_| false,
            &mut |_| slint::Image::default(),
            naygo_core::format::SizeFormat::Auto,
            naygo_core::format::DateFormat::IsoMinute,
            0,
        );
        assert_eq!(rows.len(), 4);

        // Núcleo del fix C-1: con la vista filtrada por el core, la POSICIÓN que la UI envía al
        // clicar (índice en la lista pintada) cuadra con la posición de vista del core. Esconder
        // todo menos el visible y seleccionar la "pos 0" (la única fila pintada) debe caer sobre
        // "d_visible.txt", no sobre el oculto que antes ordenaba primero (".dot").
        f.set_visibility(VisibilityFlags {
            show_hidden: false,
            show_system: false,
            hide_dotfiles: true,
        });
        f.select_single(0);
        let rows = rows_from_view(
            &f,
            &|_| false,
            &|_| false,
            &mut |_| slint::Image::default(),
            naygo_core::format::SizeFormat::Auto,
            naygo_core::format::DateFormat::IsoMinute,
            0,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "d_visible.txt");
        assert!(
            rows[0].selected,
            "la fila visible (pos 0) queda marcada, no la oculta"
        );
        assert!(rows[0].focused);
    }

    #[test]
    fn inspector_sin_foco_no_esta_presente() {
        let f = FilePaneState::new(PathBuf::from("C:/x"));
        let info = inspector_info(Some(&f), naygo_core::format::DateFormat::IsoMinute, 0);
        assert!(!info.present);
        // Sin panel: tampoco presente.
        assert!(!inspector_info(None, naygo_core::format::DateFormat::IsoMinute, 0).present);
    }

    #[test]
    fn inspector_refleja_el_foco() {
        let mut f = FilePaneState::new(PathBuf::from("C:/x"));
        f.entries = vec![mk("a.txt", false, Some(2048)), mk("dir", true, None)];
        f.select_single(0);
        let info = inspector_info(Some(&f), naygo_core::format::DateFormat::IsoMinute, 0);
        assert!(info.present);
        // El foco lo mueve select_single; el nombre debe ser uno de los dos visibles.
        assert!(info.name == "a.txt" || info.name == "dir");
        assert!(info.kind == "Archivo" || info.kind == "Carpeta");
        assert!(!info.path.is_empty());
    }

    #[test]
    fn favoritos_y_recientes_a_filas() {
        use naygo_core::favorites::Favorites;
        use naygo_core::recent_dirs::RecentDirs;
        let mut favs = Favorites::new();
        favs.toggle(std::path::Path::new("D:/Empresas/ISGroth"));
        let rows = favorite_rows(&favs, &slint::Image::default());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "ISGroth");
        assert!(rows[0].path.contains("ISGroth"));

        let mut recents = RecentDirs::new();
        recents.push(PathBuf::from("C:/Users/ng/Documents"), 50);
        let r = recent_rows(&recents, &slint::Image::default());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].label, "Documents");
    }

    #[test]
    fn favoritos_arbol_se_aplana_con_grupos_y_respeta_colapso() {
        use naygo_core::favorites::{Favorites, NodeId};
        let mut f = Favorites::new();
        f.add_favorite(std::path::Path::new("C:/uno"));
        let g = f.new_group(None, "Trabajo");
        f.add_favorite(std::path::Path::new("C:/dos"));
        f.move_node(&NodeId::favorite(std::path::Path::new("C:/dos")), Some(&g));

        // Grupo COLAPSADO: la hoja interna no aparece.
        let collapsed = |_: &str| false;
        let rows = fav_tree_rows(&f, &collapsed, &slint::Image::default());
        // raíz: "uno" (favorito) + "Trabajo" (grupo colapsado). "dos" oculto.
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .any(|r| r.name == "uno" && !r.is_group && r.depth == 0));
        let grp = rows.iter().find(|r| r.name == "Trabajo").unwrap();
        assert!(grp.is_group && grp.has_children && !grp.expanded);
        assert_eq!(grp.group_id, "1"); // segundo nodo raíz

        // Grupo EXPANDIDO: la hoja interna aparece con sangría (depth 1).
        let expanded = |np: &str| np == "Trabajo";
        let rows = fav_tree_rows(&f, &expanded, &slint::Image::default());
        assert_eq!(rows.len(), 3);
        let dos = rows.iter().find(|r| r.name == "dos").unwrap();
        assert_eq!(dos.depth, 1);
        assert!(!dos.is_group);
    }

    #[test]
    fn group_id_round_trip() {
        assert_eq!(group_id_to_str(&[0, 2, 1]), "0/2/1");
        assert_eq!(str_to_group_id("0/2/1"), vec![0, 2, 1]);
        assert_eq!(group_id_to_str(&[]), "");
        assert!(str_to_group_id("").is_empty());
    }

    #[test]
    fn historial_vacio_da_filas_vacias() {
        assert!(history_rows(&[]).is_empty());
    }

    #[test]
    fn history_rows_distingue_deshacible_de_ya_deshecho() {
        // Tres entradas: una deshacible (inverso válido contra disco real), una ya deshecha, y una
        // trabada (el inverso ya no aplica: la ruta no existe). `history_rows` las clasifica:
        //   deshacible → undoable=true, undone=false
        //   ya deshecho → undoable=false, undone=true (la UI pinta "Deshecho ✓")
        //   trabada     → undoable=false, undone=false, con motivo
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.txt");
        std::fs::write(&real, b"x").unwrap();

        let deshacible = UndoEntry {
            id: 1,
            label: "Mover".into(),
            when_epoch_secs: 0,
            // Mandar `real.txt` a la papelera es un inverso válido (el archivo existe).
            actions: vec![undo::UndoAction::TrashCreated { path: real.clone() }],
            undone: false,
        };
        let ya_deshecho = UndoEntry {
            id: 2,
            label: "Copiar".into(),
            when_epoch_secs: 0,
            actions: vec![undo::UndoAction::TrashCreated { path: real }],
            undone: true,
        };
        let trabada = UndoEntry {
            id: 3,
            label: "Mover".into(),
            when_epoch_secs: 0,
            // El inverso apunta a una ruta inexistente → validate falla → no deshacible (no deshecho).
            actions: vec![undo::UndoAction::TrashCreated {
                path: dir.path().join("no_existe.txt"),
            }],
            undone: false,
        };

        let rows = history_rows(&[deshacible, ya_deshecho, trabada]);
        assert_eq!(rows.len(), 3);
        // Deshacible.
        assert!(rows[0].undoable && !rows[0].undone, "id1 es deshacible");
        // Ya deshecho: NO deshacible, pero marcado undone (la UI lo distingue de la trabada).
        assert!(
            !rows[1].undoable && rows[1].undone,
            "id2 ya se deshizo (undone=true)"
        );
        // Trabada: ni deshacible ni deshecha, con motivo.
        assert!(
            !rows[2].undoable && !rows[2].undone,
            "id3 no es deshacible por estado, NO por haberse deshecho"
        );
        assert!(!rows[2].reason.is_empty(), "la trabada explica el motivo");
    }

    #[test]
    fn columns_info_marca_orden_y_filtro() {
        use naygo_core::columns::ColumnKind;
        use naygo_core::filter::ColumnFilter;
        let mut f = FilePaneState::new(PathBuf::from("C:/x"));
        // Orden por defecto: Name ascendente (dirs_first no cambia la clave).
        f.sort.key = naygo_core::fs_model::SortKey::Size;
        f.sort.ascending = false;
        f.table.set_filter(
            ColumnKind::Extension,
            ColumnFilter::Text {
                contains: "txt".into(),
                case_sensitive: false,
            },
        );
        let label_of = |k: ColumnKind| format!("{k:?}");
        let cols = columns_info(&f, &label_of);
        let size = cols.iter().find(|c| c.kind == 2).unwrap();
        assert_eq!(size.sort_dir, 2, "Size ordenada descendente → ▼");
        let ext = cols.iter().find(|c| c.kind == 1).unwrap();
        assert_eq!(ext.sort_dir, 0, "Extension no es la columna de orden");
        assert!(ext.has_filter, "Extension tiene filtro activo");
        let name = cols.iter().find(|c| c.kind == 0).unwrap();
        assert!(!name.has_filter, "Name sin filtro");
    }

    #[test]
    fn arbol_se_aplana_con_sangria_y_solo_desciende_expandidos() {
        use naygo_core::icon_kind::DriveKind;
        use naygo_core::tree::{DirTree, NodeOutcome};
        let drives = vec![(PathBuf::from("C:\\"), "C:\\".to_string(), DriveKind::Fixed)];
        let mut t = DirTree::from_drives(&drives);
        // Sin expandir: solo la raíz, depth 0, con chevron (Collapsed).
        let no_disk = |_: &std::path::Path| None::<naygo_core::disk::DiskUsage>;
        let rows = tree_rows(&t, &no_disk, &mut |_| slint::Image::default());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].depth, 0);
        assert!(rows[0].is_drive);
        assert!(rows[0].has_children);
        assert!(!rows[0].expanded);

        // Expandir y poblar: la raíz desciende a sus hijos con depth 1.
        t.begin_loading(std::path::Path::new("C:\\"));
        t.push_child(std::path::Path::new("C:\\"), PathBuf::from("C:\\Users"));
        t.finish_loading(std::path::Path::new("C:\\"), NodeOutcome::Done);
        let rows = tree_rows(&t, &no_disk, &mut |_| slint::Image::default());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[1].name, "Users");
        assert!(!rows[1].is_drive);
    }

    #[test]
    fn arbol_marca_la_carpeta_activa_y_el_disco() {
        use naygo_core::icon_kind::DriveKind;
        use naygo_core::tree::DirTree;
        let drives = vec![(PathBuf::from("C:\\"), "C:\\".to_string(), DriveKind::Fixed)];
        let mut t = DirTree::from_drives(&drives);
        t.set_active(PathBuf::from("C:\\"));
        let disk = |p: &std::path::Path| {
            if p == std::path::Path::new("C:\\") {
                // 42% usado: total 1000, libre 580.
                Some(naygo_core::disk::DiskUsage {
                    total: 1000,
                    free: 580,
                })
            } else {
                None
            }
        };
        let rows = tree_rows(&t, &disk, &mut |_| slint::Image::default());
        assert!(rows[0].active, "la raíz activa está marcada");
        assert_eq!(rows[0].disk_percent, 42);
        assert!(
            rows[0].disk_detail.contains('%'),
            "la raíz con dato trae texto de espacio"
        );
    }
}
