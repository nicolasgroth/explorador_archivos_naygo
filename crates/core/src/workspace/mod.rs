// Naygo — workspace: paneles independientes componibles (lógica pura).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modelo del espacio de trabajo: una colección de paneles independientes
//! (archivos / árbol / inspector), cuál está activo, y la disposición. No depende
//! de egui ni de Windows: la UI traduce esto a egui_dock.

pub mod file_pane;
pub mod layout;
pub mod nav_history;
pub mod template;

pub use file_pane::{FilePanePersist, FilePaneState};
pub use layout::{DockNode, SerializableDockLayout, SplitDir};
pub use nav_history::NavHistory;
pub use template::{
    LayoutShape, LayoutTemplate, RecentUse, TemplateDir, TemplatePane, TemplateStore,
};

use serde::{Deserialize, Serialize};

/// Identificador único y estable de un panel dentro del workspace.
/// Estable: no cambia aunque el panel se reordene en la UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u64);

/// Qué tipo de panel es.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanePurpose {
    /// Lista de archivos navegable.
    Files,
    /// Árbol de carpetas (esqueleto en Fase 2A).
    Tree,
    /// Inspector de metadatos del elemento enfocado en el panel activo.
    Inspector,
    /// Historial de operaciones con deshacer (R2b).
    History,
    /// Carpetas favoritas + recientes (fase path-bar+favoritos).
    Favorites,
    /// Vista previa liviana del archivo enfocado del panel activo (texto/imagen).
    Preview,
    /// Panel de operaciones de archivo en curso (copiar/mover) con progreso y
    /// cancelación. El panel rico y su cableado llegan en una fase posterior.
    Operations,
}

/// Un panel concreto del workspace. Solo los `Files` llevan `FilePaneState`.
#[derive(Clone, Debug)]
pub struct PaneNode {
    pub id: PaneId,
    pub purpose: PanePurpose,
    /// Estado del panel de archivos; `None` para Tree/Inspector.
    pub files: Option<FilePaneState>,
}

/// El espacio de trabajo: paneles + cuál está activo + la disposición.
#[derive(Clone, Debug)]
pub struct Workspace {
    panes: Vec<PaneNode>,
    active: Option<PaneId>,
    next_id: u64,
    /// Disposición visual (traducida a/desde egui_dock por la capa ui).
    pub layout: SerializableDockLayout,
}

impl Workspace {
    /// Workspace vacío.
    pub fn new() -> Self {
        Workspace {
            panes: Vec::new(),
            active: None,
            next_id: 0,
            layout: SerializableDockLayout::empty(),
        }
    }

    /// Agrega un panel del tipo dado y devuelve su id. Si es el primer panel,
    /// queda activo. Para `Files`, crea su `FilePaneState` parado en `dir`
    /// (ignorado para Tree/Inspector).
    pub fn add_pane(&mut self, purpose: PanePurpose, dir: std::path::PathBuf) -> PaneId {
        let id = PaneId(self.next_id);
        self.next_id += 1;
        let files = match purpose {
            PanePurpose::Files => Some(FilePaneState::new(dir)),
            _ => None,
        };
        self.panes.push(PaneNode { id, purpose, files });
        if self.active.is_none() {
            self.active = Some(id);
        }
        id
    }

    /// Quita el panel `id`. Si era el activo, reasigna el activo al primer panel
    /// `Files` restante (o a cualquier panel, o `None` si no queda ninguno).
    pub fn remove_pane(&mut self, id: PaneId) {
        self.panes.retain(|p| p.id != id);
        if self.active == Some(id) {
            self.active = self
                .panes
                .iter()
                .find(|p| p.purpose == PanePurpose::Files)
                .or_else(|| self.panes.first())
                .map(|p| p.id);
        }
    }

    /// El id del panel activo, si hay alguno.
    pub fn active_id(&self) -> Option<PaneId> {
        self.active
    }

    /// Marca `id` como activo si existe.
    pub fn set_active(&mut self, id: PaneId) {
        if self.panes.iter().any(|p| p.id == id) {
            self.active = Some(id);
        }
    }

    /// Referencia a un panel por id.
    pub fn pane(&self, id: PaneId) -> Option<&PaneNode> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Referencia mutable a un panel por id.
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut PaneNode> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// El `FilePaneState` del panel `Files` activo (lo que refleja el inspector).
    /// Si el activo no es `Files`, devuelve el primer `Files` que haya.
    pub fn active_files(&self) -> Option<&FilePaneState> {
        self.active
            .and_then(|id| self.pane(id))
            .filter(|p| p.purpose == PanePurpose::Files)
            .and_then(|p| p.files.as_ref())
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|p| p.purpose == PanePurpose::Files)
                    .and_then(|p| p.files.as_ref())
            })
    }

    /// Versión mutable de `active_files`.
    pub fn active_files_mut(&mut self) -> Option<&mut FilePaneState> {
        let target = self
            .active
            .filter(|id| {
                self.pane(*id)
                    .map(|p| p.purpose == PanePurpose::Files)
                    .unwrap_or(false)
            })
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|p| p.purpose == PanePurpose::Files)
                    .map(|p| p.id)
            })?;
        self.pane_mut(target).and_then(|p| p.files.as_mut())
    }

    /// Ids de los paneles `Files`, en orden de inserción.
    pub fn files_panes(&self) -> Vec<PaneId> {
        self.panes
            .iter()
            .filter(|p| p.purpose == PanePurpose::Files)
            .map(|p| p.id)
            .collect()
    }

    /// Ids de los paneles `Files` DISTINTOS de `origin`, en orden de inserción.
    /// Base de la regla de destino multi-panel (acciones «hacia otro panel»).
    pub fn other_files_panes(&self, origin: PaneId) -> Vec<PaneId> {
        self.panes
            .iter()
            .filter(|p| p.purpose == PanePurpose::Files && p.id != origin)
            .map(|p| p.id)
            .collect()
    }

    /// Itera los paneles (orden de inserción).
    pub fn panes(&self) -> &[PaneNode] {
        &self.panes
    }

    /// Itera los paneles mutables.
    pub fn panes_mut(&mut self) -> &mut [PaneNode] {
        &mut self.panes
    }

    /// Construye un workspace desde una plantilla: crea los paneles (mapeando
    /// índice de la plantilla → PaneId real) y arma la disposición. `home` es la
    /// carpeta para los `TemplateDir::Home`. El primer panel `Files` queda activo.
    /// Si la plantilla tiene un índice de hoja fuera de rango (p. ej. una plantilla
    /// de usuario corrupta), ese leaf se ignora con un PaneId placeholder seguro
    /// (no panica).
    pub fn from_template(
        tpl: &crate::workspace::template::LayoutTemplate,
        home: &std::path::Path,
    ) -> Self {
        use crate::workspace::layout::DockNode;
        use crate::workspace::template::{LayoutShape, TemplateDir};

        let mut w = Workspace::new();
        // Crear paneles, guardando el PaneId de cada índice.
        let mut ids: Vec<PaneId> = Vec::with_capacity(tpl.panes.len());
        for tp in &tpl.panes {
            let dir = match &tp.dir {
                TemplateDir::Home => home.to_path_buf(),
                TemplateDir::Fixed(s) => std::path::PathBuf::from(s),
            };
            ids.push(w.add_pane(tp.purpose, dir));
        }
        // Activar el primer Files.
        if let Some(first_files) = tpl
            .panes
            .iter()
            .position(|p| p.purpose == PanePurpose::Files)
        {
            w.active = Some(ids[first_files]);
        }
        // Traducir LayoutShape (índices) → SerializableDockLayout (PaneId).
        // Un índice fuera de rango cae a `ids[0]` (o no produce nodo si no hay panes).
        fn shape_to_node(shape: &LayoutShape, ids: &[PaneId]) -> Option<DockNode> {
            match shape {
                LayoutShape::Leaf(i) => {
                    let id = ids.get(*i).copied().or_else(|| ids.first().copied())?;
                    Some(DockNode::Leaf(id))
                }
                LayoutShape::Tabs { members, active } => {
                    // Traducir índices → PaneIds, ignorando los fuera de rango. Un grupo con
                    // 0 miembros válidos desaparece; con 1 se colapsa a hoja.
                    let resolved: Vec<PaneId> = members
                        .iter()
                        .filter_map(|i| ids.get(*i).copied())
                        .collect();
                    match resolved.len() {
                        0 => None,
                        1 => Some(DockNode::Leaf(resolved[0])),
                        n => Some(DockNode::Tabs {
                            members: resolved,
                            active: (*active).min(n - 1),
                        }),
                    }
                }
                LayoutShape::Split {
                    dir,
                    fraction,
                    first,
                    second,
                } => {
                    let f = shape_to_node(first, ids);
                    let s = shape_to_node(second, ids);
                    match (f, s) {
                        (Some(first), Some(second)) => Some(DockNode::Split {
                            dir: *dir,
                            fraction: *fraction,
                            first: Box::new(first),
                            second: Box::new(second),
                        }),
                        // Si un lado no se pudo construir, usar el otro.
                        (Some(only), None) | (None, Some(only)) => Some(only),
                        (None, None) => None,
                    }
                }
            }
        }
        w.layout = SerializableDockLayout {
            root: if tpl.panes.is_empty() {
                None
            } else {
                shape_to_node(&tpl.layout, &ids)
            },
        };
        w
    }

    /// Captura la disposición ACTUAL como una `LayoutTemplate` con el `name` dado (inverso de
    /// `from_template`). Los paneles se recogen en el orden en que aparecen en el árbol de
    /// disposición; los `Files` apuntan a su carpeta actual como `TemplateDir::Fixed`, los demás
    /// a `Home`. Útil para «guardar la disposición actual como plantilla».
    pub fn to_template(&self, name: &str) -> crate::workspace::template::LayoutTemplate {
        use crate::workspace::layout::DockNode;
        use crate::workspace::template::{LayoutShape, LayoutTemplate, TemplateDir, TemplatePane};

        // Recorre el árbol recogiendo PaneIds en orden y construye el LayoutShape por índice.
        // `order` mapea PaneId → índice en `panes` (en orden de aparición).
        let mut order: Vec<PaneId> = Vec::new();
        fn idx_of(order: &mut Vec<PaneId>, id: PaneId) -> usize {
            if let Some(i) = order.iter().position(|x| *x == id) {
                i
            } else {
                order.push(id);
                order.len() - 1
            }
        }
        fn node_to_shape(node: &DockNode, order: &mut Vec<PaneId>) -> LayoutShape {
            match node {
                DockNode::Leaf(id) => LayoutShape::Leaf(idx_of(order, *id)),
                DockNode::Tabs { members, active } => LayoutShape::Tabs {
                    members: members.iter().map(|id| idx_of(order, *id)).collect(),
                    active: *active,
                },
                DockNode::Split {
                    dir,
                    fraction,
                    first,
                    second,
                } => LayoutShape::Split {
                    dir: *dir,
                    fraction: *fraction,
                    first: Box::new(node_to_shape(first, order)),
                    second: Box::new(node_to_shape(second, order)),
                },
            }
        }

        let layout = match &self.layout.root {
            Some(root) => node_to_shape(root, &mut order),
            None => LayoutShape::Leaf(0),
        };
        // Construir los TemplatePane en el orden recogido del árbol.
        let panes: Vec<TemplatePane> = order
            .iter()
            .filter_map(|id| self.pane(*id))
            .map(|p| TemplatePane {
                purpose: p.purpose,
                dir: match &p.files {
                    Some(f) => TemplateDir::Fixed(f.current_dir.to_string_lossy().into_owned()),
                    None => TemplateDir::Home,
                },
            })
            .collect();
        LayoutTemplate {
            name: name.to_string(),
            builtin: false,
            favorite: false,
            panes,
            layout,
        }
    }

    /// Inserta un `PaneNode` ya construido (con su id propio), para reconstruir un workspace
    /// desde un persist. Si es el primero, queda activo. No toca `next_id` (lo fija el
    /// llamador con `set_next_id` tras insertar todos).
    pub fn push_node(&mut self, node: PaneNode) {
        let id = node.id;
        self.panes.push(node);
        if self.active.is_none() {
            self.active = Some(id);
        }
    }

    /// Fija el contador de ids (tras reconstruir desde persist), para que un `add_pane`
    /// posterior no colisione con un id ya restaurado.
    pub fn set_next_id(&mut self, next: u64) {
        self.next_id = next;
    }

    /// Reconstruye un `Workspace` desde un `WorkspacePersist` (sesión guardada). Crea cada
    /// panel con su purpose y, si es `Files`, su `FilePaneState` restaurado; conserva los
    /// `PaneId` del persist y rearma la disposición tal cual se guardó; fija el panel activo.
    /// Devuelve `None` si el layout guardado no referencia ningún panel (persist vacío o
    /// corrupto) para que el llamador caiga al arranque por defecto.
    pub fn from_persist(p: &crate::config::WorkspacePersist) -> Option<Workspace> {
        if p.layout.pane_ids().is_empty() {
            return None;
        }
        let mut w = Workspace::new();
        let files: std::collections::HashMap<PaneId, FilePanePersist> =
            p.files.iter().cloned().collect();
        let mut max_id = 0u64;
        for (id, purpose) in &p.purposes {
            let pane_files = match purpose {
                PanePurpose::Files => files
                    .get(id)
                    .cloned()
                    .map(FilePaneState::from_persist)
                    .or_else(|| {
                        // Un panel Files sin su persist (raro): arranca en HOME para no perder
                        // el panel del layout.
                        Some(FilePaneState::new(std::path::PathBuf::new()))
                    }),
                _ => None,
            };
            w.push_node(PaneNode {
                id: *id,
                purpose: *purpose,
                files: pane_files,
            });
            max_id = max_id.max(id.0);
        }
        w.set_next_id(max_id + 1);
        w.layout = p.layout.clone();
        if let Some(a) = p.active {
            w.set_active(a);
        }
        Some(w)
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn pane_id_es_comparable_y_ordenable() {
        assert_eq!(PaneId(1), PaneId(1));
        assert!(PaneId(1) < PaneId(2));
    }

    #[test]
    fn pane_purpose_round_trip_serde() {
        let json = serde_json::to_string(&PanePurpose::Files).unwrap();
        let back: PanePurpose = serde_json::from_str(&json).unwrap();
        assert_eq!(back, PanePurpose::Files);
    }

    #[test]
    fn primer_panel_queda_activo() {
        let mut w = Workspace::new();
        let id = w.add_pane(PanePurpose::Files, PathBuf::from("C:/"));
        assert_eq!(w.active_id(), Some(id));
    }

    /// Arma un `WorkspacePersist` a partir de un workspace vivo (como hará la UI al cerrar).
    fn persist_de(w: &Workspace) -> crate::config::WorkspacePersist {
        crate::config::WorkspacePersist {
            version: 1,
            layout: w.layout.clone(),
            active: w.active_id(),
            files: w
                .panes()
                .iter()
                .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.to_persist())))
                .collect(),
            purposes: w.panes().iter().map(|p| (p.id, p.purpose)).collect(),
        }
    }

    #[test]
    fn workspace_round_trip_persist() {
        let mut w = Workspace::new();
        let a = w.add_pane(PanePurpose::Files, PathBuf::from("C:/a"));
        let _tree = w.add_pane(PanePurpose::Tree, PathBuf::new());
        w.layout = SerializableDockLayout::single(a);
        w.set_active(a);

        let persist = persist_de(&w);
        let w2 = Workspace::from_persist(&persist).expect("layout no vacío → Some");

        assert_eq!(w2.panes().len(), 2);
        assert_eq!(w2.active_id(), Some(a));
        assert_eq!(
            w2.active_files().map(|f| f.current_dir.clone()),
            Some(PathBuf::from("C:/a"))
        );
        // Un panel nuevo no debe colisionar con los ids restaurados.
        let mut w3 = w2;
        let nuevo = w3.add_pane(PanePurpose::Files, PathBuf::from("C:/c"));
        assert!(w3.panes().iter().filter(|p| p.id == nuevo).count() == 1);
        assert_ne!(nuevo, a);
    }

    #[test]
    fn from_persist_con_layout_vacio_es_none() {
        let persist = crate::config::WorkspacePersist {
            version: 1,
            layout: SerializableDockLayout::empty(),
            active: None,
            files: Vec::new(),
            purposes: Vec::new(),
        };
        assert!(Workspace::from_persist(&persist).is_none());
    }

    #[test]
    fn quitar_el_activo_reasigna_a_otro_files() {
        let mut w = Workspace::new();
        let a = w.add_pane(PanePurpose::Files, PathBuf::from("C:/a"));
        let b = w.add_pane(PanePurpose::Files, PathBuf::from("C:/b"));
        w.set_active(a);
        w.remove_pane(a);
        assert_eq!(w.active_id(), Some(b));
    }

    #[test]
    fn active_files_apunta_al_panel_files_activo() {
        let mut w = Workspace::new();
        let _tree = w.add_pane(PanePurpose::Tree, PathBuf::new());
        let files = w.add_pane(PanePurpose::Files, PathBuf::from("C:/x"));
        w.set_active(files);
        assert_eq!(
            w.active_files().map(|f| f.current_dir.clone()),
            Some(PathBuf::from("C:/x"))
        );
    }

    #[test]
    fn tree_no_tiene_file_pane_state() {
        let mut w = Workspace::new();
        let t = w.add_pane(PanePurpose::Tree, PathBuf::new());
        assert!(w.pane(t).unwrap().files.is_none());
    }

    #[test]
    fn other_files_panes_excluye_el_origen_y_los_no_files() {
        let mut w = Workspace::new();
        let a = w.add_pane(PanePurpose::Files, PathBuf::from("C:/a"));
        let _tree = w.add_pane(PanePurpose::Tree, PathBuf::new());
        let b = w.add_pane(PanePurpose::Files, PathBuf::from("C:/b"));
        let c = w.add_pane(PanePurpose::Files, PathBuf::from("C:/c"));
        // Desde a: los otros Files son b y c (el árbol no cuenta), en orden.
        assert_eq!(w.other_files_panes(a), vec![b, c]);
        // files_panes incluye a todos los Files.
        assert_eq!(w.files_panes(), vec![a, b, c]);
    }

    #[test]
    fn from_template_minimalista_crea_un_files_activo() {
        let tpl = crate::workspace::template::LayoutTemplate::minimalista();
        let w = Workspace::from_template(&tpl, std::path::Path::new("C:/home"));
        assert_eq!(w.panes().len(), 1);
        assert_eq!(w.panes()[0].purpose, PanePurpose::Files);
        assert!(w.active_id().is_some());
        assert_eq!(
            w.active_files().map(|f| f.current_dir.clone()),
            Some(PathBuf::from("C:/home"))
        );
        assert_eq!(w.layout.pane_ids().len(), 1);
    }

    #[test]
    fn from_template_dual_pane_crea_cuatro_paneles() {
        let tpl = crate::workspace::template::LayoutTemplate::dual_pane();
        let w = Workspace::from_template(&tpl, std::path::Path::new("C:/home"));
        assert_eq!(w.panes().len(), 4);
        let files = w
            .panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Files)
            .count();
        assert_eq!(files, 2);
        assert_eq!(w.layout.pane_ids().len(), 4);
        let active = w.active_id().unwrap();
        assert_eq!(w.pane(active).unwrap().purpose, PanePurpose::Files);
    }

    #[test]
    fn from_template_indice_fuera_de_rango_no_panica() {
        // Plantilla corrupta: un Leaf(5) con solo 1 panel. No debe panicar.
        use crate::workspace::template::{LayoutShape, LayoutTemplate, TemplateDir, TemplatePane};
        let tpl = LayoutTemplate {
            name: "Corrupta".into(),
            builtin: false,
            favorite: false,
            panes: vec![TemplatePane {
                purpose: PanePurpose::Files,
                dir: TemplateDir::Home,
            }],
            layout: LayoutShape::Leaf(5), // fuera de rango
        };
        let w = Workspace::from_template(&tpl, std::path::Path::new("C:/home"));
        // El panel se creó; el layout cae al primer id en vez de panicar.
        assert_eq!(w.panes().len(), 1);
        assert_eq!(w.layout.pane_ids(), vec![w.panes()[0].id]);
    }

    #[test]
    fn to_template_captura_la_disposicion_actual() {
        // from_template(dual_pane) → to_template debe reproducir tipos y forma del layout.
        let tpl = crate::workspace::template::LayoutTemplate::dual_pane();
        let w = Workspace::from_template(&tpl, std::path::Path::new("C:/home"));
        let captured = w.to_template("Mi captura");
        assert_eq!(captured.name, "Mi captura");
        assert!(!captured.builtin);
        // Mismos tipos de panel en el mismo orden.
        let orig_purposes: Vec<_> = tpl.panes.iter().map(|p| p.purpose).collect();
        let cap_purposes: Vec<_> = captured.panes.iter().map(|p| p.purpose).collect();
        assert_eq!(orig_purposes, cap_purposes);
        // Re-materializar la captura da el mismo nº de paneles y hojas.
        let w2 = Workspace::from_template(&captured, std::path::Path::new("C:/home"));
        assert_eq!(w2.panes().len(), w.panes().len());
        assert_eq!(w2.layout.pane_ids().len(), w.layout.pane_ids().len());
    }
}
