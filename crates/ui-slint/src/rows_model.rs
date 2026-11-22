// Naygo — modelo estable de filas del panel de archivos, con avisos por tramo y claves por fila.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// `VecModel` solo notifica de a un elemento (`insert`/`remove`) o reinicia el modelo entero
// (`set_vec`, que mata el hover y la rueda: ver el gotcha de `set_vec`). Con 100k filas, un alta
// en el medio costaba miles de avisos o un reinicio. `RowsModel` guarda las filas en un `Vec`
// propio y avisa por TRAMO (`row_added(i, n)` / `row_removed(i, n)`), nunca con `reset`.
//
// Además guarda, en paralelo a cada fila, una `RowKey`: identidad (ruta) + firma del contenido que
// determina la `RowData`. Con ellas `sync_rows` calcula qué filas entraron, salieron o cambiaron
// comparando enteros, sin construir las 100k filas (cadenas, celdas y su `VecModel` anidado) en
// cada cambio. Solo se construyen las filas nuevas o cambiadas.

use crate::RowData;
use slint::{Model, ModelNotify, ModelTracker};
use std::cell::RefCell;
use std::collections::HashSet;

/// Identidad y contenido de una fila, derivados del estado del panel (no de la `RowData`).
/// `id` identifica la entrada (ruta); `content` cambia si cambiaría cualquier dato de la fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowKey {
    pub id: u64,
    pub content: u64,
}

/// Contenido imposible de calcular (`KeyHasher` nunca devuelve 0): marca una fila cuyo dato
/// quedó pendiente (la fila en rename inline) para que el próximo diff la reconstruya.
pub(crate) const STALE_CONTENT: u64 = 0;

/// Hash rápido (multiplicación-rotación, estilo Fx) para claves de fila. No es criptográfico ni
/// resistente a colisiones adversarias; basta para comparar 100k filas en ~1 ms. Una colisión
/// de `id` Y `content` a la vez (probabilidad ~2^-64 por par) mostraría una fila vieja hasta el
/// próximo cambio.
#[derive(Clone, Copy)]
pub(crate) struct KeyHasher(u64);

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl KeyHasher {
    pub(crate) fn new(seed: u64) -> KeyHasher {
        KeyHasher(seed ^ SEED)
    }

    #[inline]
    pub(crate) fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
    }

    pub(crate) fn add_bytes(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
        let rest = chunks.remainder();
        let mut word = [0u8; 8];
        word[..rest.len()].copy_from_slice(rest);
        self.add(u64::from_le_bytes(word));
        self.add(bytes.len() as u64);
    }

    pub(crate) fn finish(self) -> u64 {
        // Mezcla final (fmix64 de MurmurHash3) para repartir bien los bits; nunca 0.
        let mut h = self.0;
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        h ^= h >> 33;
        h | 1
    }
}

/// Un tramo de la reconciliación, en el orden de la lista nueva.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiffOp {
    /// `n` filas conservadas (misma identidad); las que cambiaron van en `KeyDiff::updates`.
    Keep(usize),
    /// `n` filas viejas que salen en la posición actual.
    Remove(usize),
    /// `n` filas nuevas que entran en la posición actual.
    Insert(usize),
}

/// Resultado de comparar claves: tramos estructurales + posiciones NUEVAS a construir.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyDiff {
    pub ops: Vec<DiffOp>,
    /// Posiciones (en la lista nueva) de filas conservadas cuyo contenido cambió.
    pub updates: Vec<usize>,
    /// Posiciones (en la lista nueva) de filas que entran.
    pub inserts: Vec<usize>,
}

impl KeyDiff {
    fn push(&mut self, op: DiffOp) {
        match (self.ops.last_mut(), op) {
            (Some(DiffOp::Keep(a)), DiffOp::Keep(b))
            | (Some(DiffOp::Remove(a)), DiffOp::Remove(b))
            | (Some(DiffOp::Insert(a)), DiffOp::Insert(b)) => *a += b,
            (_, DiffOp::Keep(0) | DiffOp::Remove(0) | DiffOp::Insert(0)) => {}
            _ => self.ops.push(op),
        }
    }

    /// Posiciones nuevas que hay que construir (altas + cambios), ordenadas.
    pub(crate) fn positions_to_build(&self) -> Vec<usize> {
        let mut all: Vec<usize> = self
            .updates
            .iter()
            .chain(self.inserts.iter())
            .copied()
            .collect();
        all.sort_unstable();
        all
    }

    pub(crate) fn is_noop(&self) -> bool {
        self.updates.is_empty() && self.ops.iter().all(|op| matches!(op, DiffOp::Keep(_)))
    }
}

/// Compara claves viejas y nuevas por identidad. Prefijo y sufijo comunes se conservan; en el
/// tramo del medio, una fila cuya identidad no existía es un alta y una que ya no existe es una
/// baja. Si el tramo cambió de orden (ordenar por otra columna), se reemplaza el resto del
/// tramo: es el caso en que de verdad cambian casi todas las filas.
pub(crate) fn diff_keys(old: &[RowKey], new: &[RowKey]) -> KeyDiff {
    let mut diff = KeyDiff::default();
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(a, b)| a.id == b.id)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a.id == b.id)
        .count();
    let keep = |diff: &mut KeyDiff, old_at: usize, new_at: usize| {
        if old[old_at].content != new[new_at].content {
            diff.updates.push(new_at);
        }
        diff.push(DiffOp::Keep(1));
    };
    for i in 0..prefix {
        keep(&mut diff, i, i);
    }
    let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
    let (mut i, mut j) = (prefix, prefix);
    if i < old_end && j < new_end {
        let old_ids: HashSet<u64> = old[i..old_end].iter().map(|k| k.id).collect();
        let new_ids: HashSet<u64> = new[j..new_end].iter().map(|k| k.id).collect();
        while i < old_end && j < new_end {
            if old[i].id == new[j].id {
                keep(&mut diff, i, j);
                i += 1;
                j += 1;
            } else if !old_ids.contains(&new[j].id) {
                diff.inserts.push(j);
                diff.push(DiffOp::Insert(1));
                j += 1;
            } else if !new_ids.contains(&old[i].id) {
                diff.push(DiffOp::Remove(1));
                i += 1;
            } else {
                // Reordenamiento: ambas filas existen en las dos listas en otro orden.
                break;
            }
        }
    }
    diff.push(DiffOp::Remove(old_end - i));
    diff.inserts.extend(j..new_end);
    diff.push(DiffOp::Insert(new_end - j));
    for k in 0..suffix {
        keep(&mut diff, old_end + k, new_end + k);
    }
    diff
}

/// Modelo de filas del panel de archivos. Ver la cabecera del módulo.
pub(crate) struct RowsModel {
    rows: RefCell<Vec<RowData>>,
    /// Claves paralelas a `rows`; `None` si alguna fila se cargó sin clave (vista profunda o
    /// filtro por tipeo), y entonces la próxima reconciliación vuelve al camino completo.
    keys: RefCell<Option<Vec<RowKey>>>,
    notify: ModelNotify,
}

impl Default for RowsModel {
    /// Vacío y con claves conocidas (la lista vacía): el primer llenado ya puede reconciliar por
    /// claves.
    fn default() -> RowsModel {
        RowsModel {
            rows: RefCell::new(Vec::new()),
            keys: RefCell::new(Some(Vec::new())),
            notify: ModelNotify::default(),
        }
    }
}

impl Model for RowsModel {
    type Data = RowData;

    fn row_count(&self) -> usize {
        self.rows.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<RowData> {
        self.rows.borrow().get(row).cloned()
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

impl RowsModel {
    /// Lee una fila sin clonarla.
    pub(crate) fn with_row<R>(&self, row: usize, f: impl FnOnce(&RowData) -> R) -> Option<R> {
        self.rows.borrow().get(row).map(f)
    }

    /// Reemplaza una fila y avisa solo esa posición.
    pub(crate) fn set_row(&self, row: usize, data: RowData) {
        let changed = {
            let mut rows = self.rows.borrow_mut();
            match rows.get_mut(row) {
                Some(slot) => {
                    *slot = data;
                    true
                }
                None => false,
            }
        };
        if changed {
            self.notify.row_changed(row);
        }
    }

    /// Agrega filas al final con UN aviso. `keys` debe ser paralelo a `rows`, o `None` si no se
    /// conocen (invalida las claves del modelo).
    pub(crate) fn append(&self, rows: Vec<RowData>, keys: Option<Vec<RowKey>>) {
        if rows.is_empty() {
            return;
        }
        let start = self.row_count();
        let count = rows.len();
        self.rows.borrow_mut().extend(rows);
        {
            let mut current = self.keys.borrow_mut();
            match (current.as_mut(), keys) {
                (Some(current), Some(keys)) if keys.len() == count => current.extend(keys),
                _ => *current = None,
            }
        }
        self.notify.row_added(start, count);
    }

    /// Quita `remove` filas en `at` e inserta `insert` en su lugar: un aviso por tramo. Las
    /// filas fuera del tramo conservan sus delegates (hover, scroll y editor de rename).
    fn splice(&self, at: usize, remove: usize, insert: Vec<RowData>) {
        if remove != 0 {
            self.rows.borrow_mut().drain(at..at + remove);
            self.notify.row_removed(at, remove);
        }
        if !insert.is_empty() {
            let count = insert.len();
            self.rows.borrow_mut().splice(at..at, insert);
            self.notify.row_added(at, count);
        }
    }

    /// Reconciliación sin claves (vista profunda, filtro por tipeo, primer llenado): conserva
    /// prefijo y sufijo iguales según `same` y reemplaza el tramo del medio con avisos por tramo.
    /// Deja las claves como desconocidas; el llamador puede fijarlas con `set_keys`.
    pub(crate) fn reconcile_by(
        &self,
        mut incoming: Vec<RowData>,
        same: impl Fn(&RowData, &RowData) -> bool,
    ) {
        let (prefix, suffix) = {
            let current = self.rows.borrow();
            let prefix = current
                .iter()
                .zip(&incoming)
                .take_while(|(a, b)| same(a, b))
                .count();
            let suffix = current[prefix..]
                .iter()
                .rev()
                .zip(incoming[prefix..].iter().rev())
                .take_while(|(a, b)| same(a, b))
                .count();
            (prefix, suffix)
        };
        let removed = self.row_count() - prefix - suffix;
        let added = incoming.len() - prefix - suffix;
        let middle: Vec<RowData> = incoming.drain(prefix..prefix + added).collect();
        self.splice(prefix, removed, middle);
        *self.keys.borrow_mut() = None;
    }

    /// Claves actuales (si se conocen).
    pub(crate) fn keys(&self) -> std::cell::Ref<'_, Option<Vec<RowKey>>> {
        self.keys.borrow()
    }

    /// Fija las claves del contenido actual; se descartan si no son paralelas a las filas.
    pub(crate) fn set_keys(&self, keys: Option<Vec<RowKey>>) {
        let keys = keys.filter(|keys| keys.len() == self.row_count());
        *self.keys.borrow_mut() = keys;
    }

    /// Actualiza la clave de una fila (si las claves se conocen).
    pub(crate) fn set_key(&self, row: usize, key: RowKey) {
        if let Some(keys) = self.keys.borrow_mut().as_mut() {
            if let Some(slot) = keys.get_mut(row) {
                *slot = key;
            }
        }
    }

    /// Olvida las claves (la próxima reconciliación usa el camino completo).
    pub(crate) fn invalidate_keys(&self) {
        *self.keys.borrow_mut() = None;
    }

    /// Aplica un `KeyDiff` en una pasada, en el orden de la lista nueva: bajas y altas con un
    /// aviso por tramo, y cada cambio de contenido con `row_changed` de su fila. `built` trae
    /// `(posición nueva, fila)` ordenado por posición, para las posiciones de
    /// `diff.positions_to_build()`. Una posición de `updates` sin fila (la fila en rename inline)
    /// se conserva tal cual y su clave queda marcada como pendiente. `new_keys` pasa a ser la
    /// clave del modelo.
    ///
    /// Al llegar a la posición nueva `at`, el prefijo `[0, at)` del modelo ya está en su forma
    /// final, así que `set_row(at + k)` dentro de un `Keep` apunta a la fila correcta.
    pub(crate) fn apply_diff(
        &self,
        diff: &KeyDiff,
        built: Vec<(usize, RowData)>,
        mut new_keys: Vec<RowKey>,
    ) {
        let mut built = built.into_iter().peekable();
        let mut updates = diff.updates.iter().copied().peekable();
        let mut at = 0;
        for op in &diff.ops {
            match *op {
                DiffOp::Keep(n) => {
                    while let Some(position) = updates.next_if(|&p| p < at + n) {
                        match built.next_if(|(p, _)| *p == position) {
                            Some((_, row)) => self.set_row(position, row),
                            None => {
                                if let Some(key) = new_keys.get_mut(position) {
                                    key.content = STALE_CONTENT;
                                }
                            }
                        }
                    }
                    at += n;
                }
                DiffOp::Remove(n) => self.splice(at, n, Vec::new()),
                DiffOp::Insert(n) => {
                    let rows: Vec<RowData> = (at..at + n)
                        .map(|position| {
                            built
                                .next_if(|(p, _)| *p == position)
                                .map(|(_, row)| row)
                                .unwrap_or_default()
                        })
                        .collect();
                    self.splice(at, 0, rows);
                    at += n;
                }
            }
        }
        *self.keys.borrow_mut() = (new_keys.len() == self.row_count()).then_some(new_keys);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::SharedString;
    use std::rc::Rc;

    fn key(id: u64, content: u64) -> RowKey {
        RowKey { id, content }
    }

    fn row(name: &str) -> RowData {
        RowData {
            name: SharedString::from(name),
            ..Default::default()
        }
    }

    /// Registra cada aviso del modelo para verificar que son por tramo y sin `reset`.
    #[derive(Default)]
    struct Recorder {
        events: RefCell<Vec<String>>,
    }

    impl slint::private_unstable_api::re_exports::ModelChangeListener for Recorder {
        fn row_changed(self: std::pin::Pin<&Self>, row: usize) {
            self.events.borrow_mut().push(format!("changed {row}"));
        }
        fn row_added(self: std::pin::Pin<&Self>, index: usize, count: usize) {
            self.events
                .borrow_mut()
                .push(format!("added {index}+{count}"));
        }
        fn row_removed(self: std::pin::Pin<&Self>, index: usize, count: usize) {
            self.events
                .borrow_mut()
                .push(format!("removed {index}+{count}"));
        }
        fn reset(self: std::pin::Pin<&Self>) {
            self.events.borrow_mut().push("reset".into());
        }
    }

    fn listen(
        model: &RowsModel,
    ) -> std::pin::Pin<
        Box<slint::private_unstable_api::re_exports::ModelChangeListenerContainer<Recorder>>,
    > {
        let listener = Box::pin(
            slint::private_unstable_api::re_exports::ModelChangeListenerContainer::new(
                Recorder::default(),
            ),
        );
        model
            .model_tracker()
            .attach_peer(listener.as_ref().model_peer());
        listener
    }

    fn names(model: &RowsModel) -> Vec<String> {
        model.iter().map(|r| r.name.to_string()).collect()
    }

    #[test]
    fn diff_detecta_altas_bajas_y_cambios_por_identidad() {
        let old = [key(1, 10), key(2, 20), key(3, 30), key(4, 40)];
        let new = [key(1, 10), key(5, 50), key(3, 31), key(4, 40), key(6, 60)];
        let diff = diff_keys(&old, &new);
        assert_eq!(
            diff.ops,
            vec![
                DiffOp::Keep(1),
                DiffOp::Insert(1),
                DiffOp::Remove(1),
                DiffOp::Keep(2),
                DiffOp::Insert(1)
            ]
        );
        assert_eq!(diff.inserts, vec![1, 4]);
        assert_eq!(diff.updates, vec![2]);
        assert_eq!(diff.positions_to_build(), vec![1, 2, 4]);
        assert!(diff_keys(&old, &old).is_noop());
    }

    #[test]
    fn diff_con_reordenamiento_reemplaza_el_tramo() {
        let old = [key(1, 1), key(2, 2), key(3, 3), key(4, 4)];
        let new = [key(1, 1), key(3, 3), key(2, 2), key(4, 4)];
        let diff = diff_keys(&old, &new);
        assert_eq!(
            diff.ops,
            vec![
                DiffOp::Keep(1),
                DiffOp::Remove(2),
                DiffOp::Insert(2),
                DiffOp::Keep(1)
            ]
        );
        assert_eq!(diff.inserts, vec![1, 2]);
    }

    #[test]
    fn insertar_y_quitar_en_medio_de_100k_avisa_por_tramo_sin_reset() {
        const N: u64 = 100_000;
        let model = Rc::new(RowsModel::default());
        let keys: Vec<RowKey> = (0..N).map(|i| key(i, i)).collect();
        model.append(
            (0..N).map(|i| row(&format!("f{i}"))).collect(),
            Some(keys.clone()),
        );
        let listener = listen(&model);

        // Alta en el medio: un solo `row_added` y solo se construye la fila nueva.
        let mut new_keys = keys.clone();
        new_keys.insert(50_000, key(N + 1, 7));
        let diff = diff_keys(&keys, &new_keys);
        assert_eq!(diff.positions_to_build(), vec![50_000]);
        let built = vec![(50_000, row("nuevo"))];
        model.apply_diff(&diff, built, new_keys.clone());
        assert_eq!(model.row_count(), N as usize + 1);
        assert_eq!(model.row_data(50_000).unwrap().name, "nuevo");
        assert_eq!(model.row_data(50_001).unwrap().name, "f50000");

        // Baja de un tramo en el medio: un solo `row_removed`.
        let mut after: Vec<RowKey> = new_keys.clone();
        after.drain(10..20);
        let diff = diff_keys(&new_keys, &after);
        assert!(diff.positions_to_build().is_empty());
        model.apply_diff(&diff, Vec::new(), after);
        assert_eq!(model.row_count(), N as usize - 9);
        assert_eq!(model.row_data(10).unwrap().name, "f20");

        assert_eq!(
            *listener.events.borrow(),
            vec!["added 50000+1".to_string(), "removed 10+10".to_string()]
        );
    }

    #[test]
    fn append_y_reconcile_avisan_por_tramo() {
        let model = RowsModel::default();
        model.append(
            vec![row("a"), row("b"), row("e")],
            Some(vec![key(1, 1), key(2, 2), key(5, 5)]),
        );
        let listener = listen(&model);
        model.append(vec![row("f"), row("g")], Some(vec![key(6, 6), key(7, 7)]));
        assert_eq!(model.keys().as_ref().map(Vec::len), Some(5));
        model.reconcile_by(
            vec![row("a"), row("c"), row("d"), row("e"), row("f"), row("g")],
            |a, b| a.name == b.name,
        );
        assert_eq!(names(&model), ["a", "c", "d", "e", "f", "g"]);
        assert!(
            model.keys().is_none(),
            "sin claves tras reconciliar por contenido"
        );
        assert_eq!(
            *listener.events.borrow(),
            vec![
                "added 3+2".to_string(),
                "removed 1+1".to_string(),
                "added 1+2".to_string()
            ]
        );
    }

    #[test]
    fn cambio_sin_fila_construida_queda_pendiente() {
        let model = RowsModel::default();
        model.append(vec![row("a"), row("b")], Some(vec![key(1, 1), key(2, 2)]));
        let new_keys = vec![key(1, 1), key(2, 3)];
        let diff = diff_keys(model.keys().as_ref().unwrap(), &new_keys);
        model.apply_diff(&diff, Vec::new(), new_keys);
        assert_eq!(model.row_data(1).unwrap().name, "b", "la fila se conserva");
        assert_eq!(model.keys().as_ref().unwrap()[1].content, STALE_CONTENT);
    }

    #[test]
    fn hasher_distingue_bytes_y_largos() {
        let hash = |bytes: &[u8]| {
            let mut h = KeyHasher::new(0);
            h.add_bytes(bytes);
            h.finish()
        };
        assert_ne!(hash(b"abc"), hash(b"abd"));
        assert_ne!(hash(b"abc"), hash(b"abc\0"));
        assert_ne!(hash(b"12345678a"), hash(b"12345678b"));
        assert_eq!(hash(b"same"), hash(b"same"));
        assert_ne!(hash(b""), STALE_CONTENT);
    }
}
