// Naygo — contexto compartido para el cableado de callbacks de la UI (módulos callbacks_*).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Las dependencias que los handlers de Slint capturan por clones (controlador, closures de
// sincronización, estado de paleta/confirmaciones pendientes) se agrupan aquí para que cada
// `wire_*` reciba una sola referencia en vez de una docena de parámetros. Todo es `Rc`/células
// compartidas: clonar un `WireCtx` nunca duplica estado.

use crate::workspace_ctrl::WorkspaceCtrl;
use naygo_core::workspace::layout::Rect;
use std::cell::RefCell;
use std::rc::Rc;

/// Dependencias compartidas del cableado de callbacks (ver módulos `callbacks_*`).
pub(crate) struct WireCtx {
    pub ctrl: Rc<RefCell<WorkspaceCtrl>>,
    pub sync_rows: Rc<dyn Fn()>,
    pub sync_layout: Rc<dyn Fn()>,
    pub start_timer: Rc<dyn Fn()>,
    pub area_of: Rc<dyn Fn() -> Rect>,
    /// Comandos vigentes de la paleta (Ctrl+P) mientras está abierta.
    pub palette_cmds: Rc<RefCell<Vec<naygo_core::palette::Command>>>,
    /// Índice del COMANDO que cada FILA visible de la paleta ejecuta (tabla paralela).
    pub palette_cmd_indices: Rc<RefCell<Vec<usize>>>,
    /// Path pendiente de expulsar mientras el modal de confirmación está abierto.
    pub pending_eject: Rc<RefCell<Option<String>>>,
    /// Ids de los paneles a soltar (cerrar watcher) antes de expulsar, mientras el modal está abierto.
    pub pending_eject_panes: Rc<RefCell<Vec<u64>>>,
    /// Id de la entrada del historial a DESHACER mientras el popup de confirmación (MessageVm
    /// kind 5) está abierto.
    pub pending_undo: Rc<RefCell<Option<u64>>>,
}
