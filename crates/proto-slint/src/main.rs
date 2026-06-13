// Naygo — PROTOTIPO de medición en Slint (renderizador por software, sin GPU).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Objetivo ÚNICO: medir el consumo de CPU de Slint en modo retenido + software
// renderer, listando carpetas reales con naygo-core, para decidir si reescribir la
// capa ui sobre Slint resuelve el problema de las máquinas sin GPU. NO es el producto.
//
// Forzar el renderizador por software (como en una VM sin GPU) con:
//   SLINT_BACKEND=winit-software
// En PowerShell:  $env:SLINT_BACKEND="winit-software"; cargo run -p naygo-proto-slint

use naygo_core::cancel::CancellationToken;
use naygo_core::fs_model::{Entry, EntryKind};
use naygo_core::listing::{spawn_listing, ListingMsg};
use slint::{ModelRc, SharedString, VecModel};
use std::path::PathBuf;
use std::rc::Rc;

slint::include_modules!();

/// Lista una carpeta de forma SÍNCRONA usando el motor real (naygo-core): drena el
/// worker hasta Done/Error. Para el prototipo basta bloquear (el producto ya hace esto
/// async); aquí sólo nos importa medir el RENDER, no la concurrencia.
fn list_dir(dir: &PathBuf) -> Vec<Entry> {
    let token = CancellationToken::new();
    let (rx, _handle) = spawn_listing(dir.clone(), token);
    let mut entries = Vec::new();
    while let Ok(msg) = rx.recv() {
        match msg {
            ListingMsg::Entry(e) => entries.push(e),
            ListingMsg::Done | ListingMsg::Cancelled | ListingMsg::Error(_) => break,
        }
    }
    // Orden simple: carpetas primero, luego por nombre (sólo para que se vea ordenado).
    entries.sort_by(|a, b| {
        let da = a.kind == EntryKind::Directory;
        let db = b.kind == EntryKind::Directory;
        db.cmp(&da).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

/// Convierte las entries del core en el modelo de filas que consume el .slint.
fn rows_for(entries: &[Entry]) -> Vec<RowData> {
    entries
        .iter()
        .map(|e| {
            let ext = e
                .path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            RowData {
                name: SharedString::from(e.name.as_str()),
                ext: SharedString::from(ext.as_str()),
                is_dir: e.kind == EntryKind::Directory,
            }
        })
        .collect()
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    // Carpeta inicial: el home del usuario (o C:\ como fallback).
    let start = dirs_home().unwrap_or_else(|| PathBuf::from("C:/"));

    // Estado compartido entre callbacks: la carpeta actual y sus entries.
    let state = Rc::new(std::cell::RefCell::new(list_dir(&start)));
    let cur = Rc::new(std::cell::RefCell::new(start.clone()));

    // Modelo de filas (Slint lo observa; al reemplazarlo, la ListView se re-puebla).
    let model: Rc<VecModel<RowData>> = Rc::new(VecModel::from(rows_for(&state.borrow())));
    ui.set_rows(ModelRc::from(model.clone()));
    ui.set_current_path(SharedString::from(start.to_string_lossy().as_ref()));

    // Doble clic en una fila: si es carpeta, navegar.
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        let cur = cur.clone();
        let model = model.clone();
        ui.on_row_activated(move |i| {
            let idx = i as usize;
            let entries = state.borrow();
            let Some(e) = entries.get(idx) else { return };
            if e.kind != EntryKind::Directory {
                return;
            }
            let target = e.path.clone();
            drop(entries);
            let new_entries = list_dir(&target);
            let new_rows = rows_for(&new_entries);
            *state.borrow_mut() = new_entries;
            *cur.borrow_mut() = target.clone();
            // Reemplazar el contenido del modelo (re-puebla la ListView).
            model.set_vec(new_rows);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_current_path(SharedString::from(target.to_string_lossy().as_ref()));
            }
        });
    }

    // Subir al directorio padre.
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        let cur = cur.clone();
        let model = model.clone();
        ui.on_go_up(move || {
            let parent = cur.borrow().parent().map(|p| p.to_path_buf());
            let Some(parent) = parent else { return };
            let new_entries = list_dir(&parent);
            let new_rows = rows_for(&new_entries);
            *state.borrow_mut() = new_entries;
            *cur.borrow_mut() = parent.clone();
            model.set_vec(new_rows);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_current_path(SharedString::from(parent.to_string_lossy().as_ref()));
            }
        });
    }

    ui.run()
}

/// Home del usuario en Windows vía %USERPROFILE% (sin dependencias extra).
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}
