// Naygo — orden y filas visibles compartidas por la vista profunda.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use naygo_core::columns::ColumnKind;
use naygo_core::filter::{ColumnFilter, VisibilityFlags};
use naygo_core::fs_model::{Entry, SortSpec};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

pub(super) struct DeepView {
    items_signature: (u64, u64),
    visibility: VisibilityFlags,
    filters: BTreeMap<ColumnKind, ColumnFilter>,
    indices: Arc<[usize]>,
}

impl WorkspaceCtrl {
    pub(super) fn deep_view_indices(&self, id: PaneId) -> Option<Arc<[usize]>> {
        self.deep_view_indices_impl(id, true)
    }

    pub(super) fn cached_deep_view_indices(&self, id: PaneId) -> Option<Arc<[usize]>> {
        self.deep_view_indices_impl(id, false)
    }

    fn deep_view_indices_impl(&self, id: PaneId, rebuild: bool) -> Option<Arc<[usize]>> {
        let job = self.deep_job.as_ref().filter(|job| job.pane == id)?;
        let files = self.ws.pane(id)?.files.as_ref()?;
        let filters = &files.table.filters;
        let visibility = self.visibility_flags();
        let items_signature = job.items.signature();
        let mut cache = job.deep_view.borrow_mut();
        if let Some(view) = cache.as_ref().filter(|view| {
            view.items_signature == items_signature
                && view.visibility == visibility
                && &view.filters == filters
        }) {
            return Some(view.indices.clone());
        }
        if !rebuild {
            return None;
        }
        let indices: Arc<[usize]> = job
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, (entry, _))| {
                (visibility.allows(entry) && naygo_core::filter::matches(entry, filters))
                    .then_some(index)
            })
            .collect();
        *cache = Some(DeepView {
            items_signature,
            visibility,
            filters: filters.clone(),
            indices: indices.clone(),
        });
        Some(indices)
    }
}

/// Ordena cada grupo de hermanos y recorre sus ramas sin recursión de stack. Se llama
/// al ordenar explícitamente. Los renombres y snapshots mantienen luego ese orden por ruta.
pub(super) fn sort_siblings(entries: &mut Vec<(Entry, u32)>, root: &Path, spec: SortSpec) {
    let mut siblings = HashMap::<&Path, Vec<usize>>::new();
    for (index, (entry, _)) in entries.iter().enumerate() {
        if let Some(parent) = entry.path.parent() {
            siblings.entry(parent).or_default().push(index);
        }
    }
    for group in siblings.values_mut() {
        group.sort_by(|&a, &b| naygo_core::sort::cmp_entries(&entries[a].0, &entries[b].0, &spec));
    }
    let mut stack = siblings.remove(root).unwrap_or_default();
    stack.reverse();
    let mut order = Vec::with_capacity(entries.len());
    while let Some(index) = stack.pop() {
        order.push(index);
        if let Some(children) = siblings.remove(entries[index].0.path.as_path()) {
            stack.extend(children.into_iter().rev());
        }
    }
    let mut previous: Vec<_> = std::mem::take(entries).into_iter().map(Some).collect();
    entries.extend(order.into_iter().filter_map(|index| previous[index].take()));
    // Una rama sin su padre (recorrido parcial) conserva su orden de llegada.
    entries.extend(previous.into_iter().flatten());
}
