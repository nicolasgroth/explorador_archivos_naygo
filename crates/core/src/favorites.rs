// Naygo — favoritos: carpetas ancladas por el usuario (puro, persistente en JSON).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modelo PURO de las carpetas favoritas, ahora como un ÁRBOL de grupos anidados.
//! Cada nodo es o bien una carpeta favorita (`FavNode::Favorite`) o un grupo con
//! hijos (`FavNode::Group`). El recorrido pre-orden de las hojas define el orden de
//! los atajos `Ctrl+1..9`. Sin egui ni Windows. Lo consumen la path-bar (estrella
//! ☆/★), el panel Favoritos, la sección anclada del árbol, los atajos de salto y la
//! paleta (Ctrl+P). Persistencia en `<config>/favorites.json`.
//!
//! Carga tolerante: corrupto o ausente → árbol vacío, nunca cae la app. Además se
//! MIGRA el formato plano antiguo (`{"items":[{"path","label"}]}`): cada favorito
//! viejo pasa a ser un `FavNode::Favorite` en la raíz, conservando orden y etiqueta.
//!
//! ## Identificación de nodos
//!
//! - Un **favorito** se identifica por su ruta ([`NodeId::Favorite`]). Las rutas son
//!   únicas en el árbol (igual que en el modelo plano anterior), así que sirven de
//!   clave natural para `contains`/`remove`/`move_node`.
//! - Un **grupo** se identifica por su "ruta de índices" ([`GroupId`] = `Vec<usize>`):
//!   la secuencia de posiciones de hijo desde la raíz hasta el grupo (p.ej. `[0]` es
//!   el primer nodo raíz; `[0, 2]` es el tercer hijo de ese grupo). Es simple, no
//!   requiere inventar ni persistir ids estables, y se mantiene válida mientras no se
//!   reordene el árbol entre la obtención del id y su uso (uso transaccional típico de
//!   la UI: obtener id → operar de inmediato).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Una carpeta favorita: ruta + etiqueta visible (hoy el nombre de la carpeta;
/// editable a futuro). Es el tipo que devuelve [`Favorites::list_flat`] y el que usa
/// la migración del formato plano antiguo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Favorite {
    pub path: PathBuf,
    pub label: String,
}

/// Un nodo del árbol de favoritos: una carpeta favorita o un grupo con hijos.
///
/// Se serializa con una etiqueta `kind` (`"favorite"` | `"group"`) para que el JSON
/// sea legible y robusto.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FavNode {
    /// Hoja: una carpeta anclada.
    Favorite { path: PathBuf, label: String },
    /// Rama: un grupo con nombre y sus hijos (favoritos u otros grupos).
    Group { name: String, children: Vec<FavNode> },
}

/// Identificador de un nodo del árbol para operaciones de movimiento.
///
/// - [`NodeId::Favorite`]: por ruta (clave única).
/// - [`NodeId::Group`]: por ruta de índices (ver doc del módulo).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeId {
    Favorite(PathBuf),
    Group(GroupId),
}

impl NodeId {
    /// Construye un [`NodeId::Favorite`] desde una ruta.
    pub fn favorite(path: &Path) -> Self {
        NodeId::Favorite(path.to_path_buf())
    }

    /// Construye un [`NodeId::Group`] desde una ruta de índices.
    pub fn group(id: GroupId) -> Self {
        NodeId::Group(id)
    }
}

/// Ruta de índices que ubica un grupo en el árbol (ver doc del módulo).
pub type GroupId = Vec<usize>;

/// Árbol de favoritos (formato nuevo). Reemplaza la lista plana anterior.
///
/// `#[serde(deny_unknown_fields)]` es clave para la migración: el JSON viejo trae la
/// clave `items` (desconocida aquí), así que su parse como `Favorites` FALLA y
/// [`Favorites::from_json`] cae al camino de migración. Un `{ "roots": [] }` válido
/// sí se acepta como árbol nuevo (vacío) sin confundirse con el viejo.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Favorites {
    #[serde(default)]
    roots: Vec<FavNode>,
}

/// Etiqueta por defecto de una ruta: el nombre de la carpeta, o la ruta completa
/// para raíces de unidad ("D:\" no tiene `file_name`).
fn default_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// ¿Algún nodo del subárbol (incluidas las hojas dentro de grupos) tiene esta ruta?
fn subtree_contains(nodes: &[FavNode], path: &Path) -> bool {
    nodes.iter().any(|n| match n {
        FavNode::Favorite { path: p, .. } => p == path,
        FavNode::Group { children, .. } => subtree_contains(children, path),
    })
}

/// Quita la hoja con esta ruta en cualquier punto del subárbol (recursivo). Los
/// grupos se conservan aunque queden vacíos (los borra el usuario, no `remove`).
fn subtree_remove(nodes: &mut Vec<FavNode>, path: &Path) {
    nodes.retain(|n| !matches!(n, FavNode::Favorite { path: p, .. } if p == path));
    for n in nodes.iter_mut() {
        if let FavNode::Group { children, .. } = n {
            subtree_remove(children, path);
        }
    }
}

/// Recorre el subárbol en pre-orden y empuja cada hoja a `out` (orden de `Ctrl+1..9`).
fn flatten_into(nodes: &[FavNode], out: &mut Vec<Favorite>) {
    for n in nodes {
        match n {
            FavNode::Favorite { path, label } => out.push(Favorite {
                path: path.clone(),
                label: label.clone(),
            }),
            FavNode::Group { children, .. } => flatten_into(children, out),
        }
    }
}

impl Favorites {
    pub fn new() -> Self {
        Self::default()
    }

    /// Los nodos de la raíz del árbol (para que la UI dibuje la estructura).
    pub fn roots(&self) -> &[FavNode] {
        &self.roots
    }

    /// ¿La ruta ya es favorita (en cualquier grupo)?
    pub fn contains(&self, path: &Path) -> bool {
        subtree_contains(&self.roots, path)
    }

    /// Agrega la ruta como favorito en la raíz si no estaba en ninguna parte del
    /// árbol (etiqueta = nombre del último componente). No-op si ya existe.
    pub fn add_favorite(&mut self, path: &Path) {
        if !self.contains(path) {
            self.roots.push(FavNode::Favorite {
                path: path.to_path_buf(),
                label: default_label(path),
            });
        }
    }

    /// Agrega la ruta (a la raíz) si no estaba, o la quita si ya estaba (en cualquier
    /// grupo). Devuelve `true` si quedó agregada, `false` si quedó quitada. Lo usa la
    /// estrella ☆/★ de la path-bar.
    pub fn toggle(&mut self, path: &Path) -> bool {
        if self.contains(path) {
            self.remove(path);
            false
        } else {
            self.add_favorite(path);
            true
        }
    }

    /// Quita la ruta esté donde esté en el árbol (no-op si no estaba). Los grupos no
    /// se borran aunque queden vacíos.
    pub fn remove(&mut self, path: &Path) {
        subtree_remove(&mut self.roots, path);
    }

    /// Crea un grupo vacío con `name` dentro de `parent` (o en la raíz si es `None`).
    /// Devuelve el [`GroupId`] del grupo recién creado. Si `parent` no existe o no es
    /// un grupo, lo crea en la raíz como degradación segura.
    pub fn new_group(&mut self, parent: Option<&GroupId>, name: &str) -> GroupId {
        let node = FavNode::Group {
            name: name.to_string(),
            children: Vec::new(),
        };
        match parent.and_then(|id| children_of_mut(&mut self.roots, id)) {
            Some(children) => {
                children.push(node);
                let mut id = parent.cloned().unwrap_or_default();
                id.push(children.len() - 1);
                id
            }
            None => {
                self.roots.push(node);
                vec![self.roots.len() - 1]
            }
        }
    }

    /// Renombra el grupo identificado por `id`. No-op si no existe o no es un grupo.
    pub fn rename_group(&mut self, id: &GroupId, name: &str) {
        if let Some(FavNode::Group { name: n, .. }) = node_at_mut(&mut self.roots, id) {
            *n = name.to_string();
        }
    }

    /// Mueve un nodo (favorito por ruta, o grupo por id) a `new_parent` (o a la raíz
    /// si es `None`). No-op si el nodo no existe, si el destino no es un grupo, o si
    /// se intenta mover un grupo dentro de sí mismo o de un descendiente.
    pub fn move_node(&mut self, node: &NodeId, new_parent: Option<&GroupId>) {
        // No mover un grupo dentro de sí mismo o de un descendiente.
        if let (NodeId::Group(id), Some(dest)) = (node, new_parent) {
            if dest.len() >= id.len() && dest[..id.len()] == id[..] {
                return;
            }
        }

        // Extraer el nodo de su lugar actual, anotando DÓNDE estaba: quitar un nodo
        // desplaza los índices de sus hermanos posteriores, así que el `GroupId` del
        // destino puede quedar obsoleto. Lo corregimos antes de reinsertar.
        let (taken, removed_from) = match node {
            NodeId::Favorite(path) => match extract_favorite(&mut self.roots, path) {
                Some(found) => found,
                None => return,
            },
            NodeId::Group(id) => match extract_group(&mut self.roots, id) {
                Some(found) => (found, id.clone()),
                None => return,
            },
        };

        // Ajustar el id del destino por el desplazamiento que provocó la extracción.
        let adjusted = new_parent.map(|id| adjust_after_removal(id, &removed_from));

        // Reinsertar en el destino. Si el destino ya no es válido, devolver a la raíz
        // para no perder el nodo.
        match adjusted
            .as_ref()
            .and_then(|id| children_of_mut(&mut self.roots, id))
        {
            Some(children) => children.push(taken),
            None => self.roots.push(taken),
        }
    }

    /// Aplana el árbol a la lista de favoritos en pre-orden (orden de `Ctrl+1..9`).
    pub fn list_flat(&self) -> Vec<Favorite> {
        let mut out = Vec::new();
        flatten_into(&self.roots, &mut out);
        out
    }

    /// Alias histórico de [`Favorites::list_flat`]: los favoritos aplanados, en orden
    /// de usuario (índice 0 = `Ctrl+1`). Devuelve un `Vec` propio (el árbol no es un
    /// slice contiguo).
    pub fn list(&self) -> Vec<Favorite> {
        self.list_flat()
    }

    /// Serializa a JSON (pretty: archivo diminuto e inspeccionable). Forma `{roots:[…]}`.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Carga tolerante con migración:
    /// 1. Intenta el formato nuevo (árbol `{roots:[…]}`, parse estricto).
    /// 2. Si falla, intenta el formato plano viejo (`{items:[{path,label}]}`) y migra
    ///    cada favorito a una hoja en la raíz.
    /// 3. Si todo falla (corrupto/ausente) → árbol vacío.
    pub fn from_json(s: &str) -> Self {
        // (1) Formato nuevo (árbol). `deny_unknown_fields` hace que el JSON viejo
        // (clave `items`) NO se acepte aquí, forzando la migración del paso (2).
        if let Ok(nuevo) = serde_json::from_str::<Favorites>(s) {
            return nuevo;
        }
        // (2) Formato viejo (lista plana) → migrar a roots.
        #[derive(Deserialize)]
        struct Viejo {
            items: Vec<Favorite>,
        }
        if let Ok(v) = serde_json::from_str::<Viejo>(s) {
            return Favorites {
                roots: v
                    .items
                    .into_iter()
                    .map(|f| FavNode::Favorite {
                        path: f.path,
                        label: f.label,
                    })
                    .collect(),
            };
        }
        // (3) Corrupto o ausente.
        Favorites::default()
    }
}

/// Devuelve `&mut` al nodo en la ruta de índices `id` (o `None` si no existe).
fn node_at_mut<'a>(roots: &'a mut [FavNode], id: &GroupId) -> Option<&'a mut FavNode> {
    let (first, rest) = id.split_first()?;
    let node = roots.get_mut(*first)?;
    if rest.is_empty() {
        return Some(node);
    }
    match node {
        FavNode::Group { children, .. } => node_at_mut(children, &rest.to_vec()),
        FavNode::Favorite { .. } => None,
    }
}

/// Devuelve `&mut` al vector de hijos del grupo en `id` (o `None` si no existe o no
/// es un grupo).
fn children_of_mut<'a>(roots: &'a mut [FavNode], id: &GroupId) -> Option<&'a mut Vec<FavNode>> {
    match node_at_mut(roots, id)? {
        FavNode::Group { children, .. } => Some(children),
        FavNode::Favorite { .. } => None,
    }
}

/// Saca (remueve y devuelve) la primera hoja con esta ruta del subárbol, junto con
/// la ruta de índices desde donde se removió (para ajustar ids dependientes).
fn extract_favorite(nodes: &mut Vec<FavNode>, path: &Path) -> Option<(FavNode, GroupId)> {
    if let Some(pos) = nodes
        .iter()
        .position(|n| matches!(n, FavNode::Favorite { path: p, .. } if p == path))
    {
        return Some((nodes.remove(pos), vec![pos]));
    }
    for (i, n) in nodes.iter_mut().enumerate() {
        if let FavNode::Group { children, .. } = n {
            if let Some((found, mut sub)) = extract_favorite(children, path) {
                let mut full = vec![i];
                full.append(&mut sub);
                return Some((found, full));
            }
        }
    }
    None
}

/// Ajusta el id de un grupo destino tras remover un nodo de `removed_from`.
///
/// Si el nodo removido era un hermano ANTERIOR del destino dentro del MISMO padre, el
/// destino "subió" una posición; decrementamos ese índice. En cualquier otro caso el
/// destino no cambia.
fn adjust_after_removal(dest: &GroupId, removed_from: &GroupId) -> GroupId {
    // El desplazamiento solo afecta si comparten el prefijo del padre y difieren en el
    // último índice del nivel de la remoción.
    if removed_from.is_empty() {
        return dest.clone();
    }
    let parent_len = removed_from.len() - 1;
    if dest.len() > parent_len
        && dest[..parent_len] == removed_from[..parent_len]
        && dest[parent_len] > removed_from[parent_len]
    {
        let mut adj = dest.clone();
        adj[parent_len] -= 1;
        return adj;
    }
    dest.clone()
}

/// Saca (remueve y devuelve) el grupo en la ruta de índices `id`.
fn extract_group(roots: &mut Vec<FavNode>, id: &GroupId) -> Option<FavNode> {
    let (last, parent_id) = id.split_last()?;
    if parent_id.is_empty() {
        if *last < roots.len() {
            return Some(roots.remove(*last));
        }
        return None;
    }
    let children = children_of_mut(roots, &parent_id.to_vec())?;
    if *last < children.len() {
        Some(children.remove(*last))
    } else {
        None
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
    fn arbol_agrega_y_contiene() {
        let mut f = Favorites::default();
        f.add_favorite(&p("D:/a"));
        assert!(f.contains(&p("D:/a")));
        f.remove(&p("D:/a"));
        assert!(!f.contains(&p("D:/a")));
    }

    #[test]
    fn grupos_y_mover() {
        let mut f = Favorites::default();
        f.add_favorite(&p("D:/a"));
        let g = f.new_group(None, "Trabajo"); // grupo en la raíz → devuelve su id
        f.move_node(&NodeId::favorite(&p("D:/a")), Some(&g)); // mueve el favorito al grupo
                                                              // sigue existiendo y list_flat lo recorre
        assert!(f.contains(&p("D:/a")));
        let flat = f.list_flat();
        assert!(flat.iter().any(|fav| fav.path == p("D:/a")));
        // El grupo es ahora el único nodo raíz y contiene al favorito.
        assert_eq!(f.roots().len(), 1);
        assert!(matches!(
            &f.roots()[0],
            FavNode::Group { name, children } if name == "Trabajo" && children.len() == 1
        ));
    }

    #[test]
    fn renombrar_grupo() {
        let mut f = Favorites::default();
        let g = f.new_group(None, "Viejo");
        f.rename_group(&g, "Nuevo");
        assert!(f
            .roots()
            .iter()
            .any(|n| matches!(n, FavNode::Group { name, .. } if name == "Nuevo")));
    }

    #[test]
    fn migra_formato_plano_viejo() {
        let viejo = r#"{"items":[{"path":"D:/x","label":"x"}]}"#;
        let f = Favorites::from_json(viejo);
        assert!(f.contains(&p("D:/x")), "el favorito viejo debe migrar al árbol");
    }

    #[test]
    fn round_trip_arbol() {
        let mut f = Favorites::default();
        f.add_favorite(&p("D:/a"));
        let g = f.new_group(None, "G");
        f.move_node(&NodeId::favorite(&p("D:/a")), Some(&g));
        let json = f.to_json();
        let f2 = Favorites::from_json(&json);
        assert!(f2.contains(&p("D:/a")));
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
        let labels: Vec<String> = f.list().iter().map(|x| x.label.clone()).collect();
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
    fn arbol_vacio_no_se_confunde_con_viejo() {
        // `{roots:[]}` debe parsear como árbol nuevo vacío, no caer a migración.
        let f = Favorites::from_json(r#"{"roots":[]}"#);
        assert!(f.list().is_empty());
        assert!(f.roots().is_empty());
    }

    #[test]
    fn mover_grupo_dentro_de_si_mismo_es_noop() {
        let mut f = Favorites::default();
        let g = f.new_group(None, "G");
        f.add_favorite(&p("D:/a"));
        f.move_node(&NodeId::favorite(&p("D:/a")), Some(&g));
        // Intentar mover G dentro de su propio hijo no debe perder nada.
        let antes = f.list_flat();
        f.move_node(&NodeId::group(g.clone()), Some(&g));
        assert_eq!(f.list_flat(), antes);
        assert!(f.contains(&p("D:/a")));
    }

    #[test]
    fn grupo_anidado_y_aplanado_preorden() {
        let mut f = Favorites::default();
        f.add_favorite(&p("C:/1"));
        let g = f.new_group(None, "G");
        f.move_node(&NodeId::favorite(&p("C:/1")), Some(&g));
        let sub = f.new_group(Some(&g), "Sub");
        f.add_favorite(&p("C:/2"));
        f.move_node(&NodeId::favorite(&p("C:/2")), Some(&sub));
        // Pre-orden: dentro de G primero la hoja 1, luego Sub con la hoja 2.
        let flat: Vec<PathBuf> = f.list_flat().into_iter().map(|x| x.path).collect();
        assert_eq!(flat, vec![p("C:/1"), p("C:/2")]);
    }

    #[test]
    fn favorites_path_apunta_al_json() {
        let dir = p("C:\\config");
        assert_eq!(favorites_path(&dir), p("C:\\config\\favorites.json"));
    }
}
