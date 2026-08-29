// Naygo — cableado de callbacks del listado de archivos (clics, drag, orden y columnas).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers del panel Files: clic/doble-clic/clic-medio en filas, arrastre OLE hacia afuera,
// rubber-band, orden por columna, y el menú/editor de columna (clic derecho en el header).
// Extraídos de `main.rs` sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::wire::WireCtx;
use crate::*;

/// Registra en la AppWindow los callbacks del listado de archivos y del menú de columnas.
pub(crate) fn wire_listing(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_rows,
        sync_layout,
        start_timer,
        ..
    } = ctx;
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let sync_layout = sync_layout.clone();
        ui.on_row_clicked(move |id, pos, ctrl_mod, shift_mod| {
            // El doble-clic se detecta en Rust (no en Slint): on_row_clicked devuelve true
            // si este clic completó un doble-clic, en cuyo caso navegó/abrió. Los modificadores
            // (ctrl_mod/shift_mod) vienen del PROPIO evento de clic, no del estado sticky del
            // controlador (que podía quedar pegado tras un modal/drag que se tragó el key-release).
            let navigated = ctrl.borrow_mut().on_row_clicked(
                PaneId(id as u64),
                pos as usize,
                ctrl_mod,
                shift_mod,
                std::time::Instant::now(),
            );
            // Cambiar el foco/navegar puede disparar un preview o cambiar el layout.
            start_timer();
            if navigated {
                sync_layout();
            } else {
                sync_rows();
            }
        });
    }
    // Toggle de vista profunda (botón de la barra del panel Files).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_toggle_deep(move |id| {
            ctrl.borrow_mut().deep_toggle(PaneId(id as u64));
            start_timer();
            sync_rows();
        });
    }
    {
        // Doble clic NATIVO de Slint (cronometrado por el SO): camino primario para abrir
        // carpetas, robusto ante la latencia del hilo de UI bajo render por software (caso
        // VM). La detección por tiempo en Rust (en on_row_clicked) queda de respaldo; el
        // controlador evita la doble navegación con una marca temporal.
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_row_double_clicked(move |id, pos| {
            if ctrl
                .borrow_mut()
                .on_row_double_clicked_native(PaneId(id as u64), pos as usize)
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        // Clic-medio (rueda) sobre una fila-carpeta: abre SIEMPRE en un panel nuevo (split).
        // Sobre un archivo no hace nada (on_row_middle_clicked devuelve false).
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_row_middle_clicked(move |id, pos| {
            if ctrl
                .borrow_mut()
                .on_row_middle_clicked(PaneId(id as u64), pos as usize)
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        // Arrastre OLE hacia afuera (Fase 5C): saca los archivos seleccionados del panel
        // hacia el Explorer/escritorio/otra app —o a otro panel de Naygo—. `start_drag` es
        // BLOQUEANTE: `DoDragDrop` corre su propio bucle modal de mensajes de Windows hasta
        // que el usuario suelta. Ese bucle modal RE-ENTRA los callbacks de Slint (el timer
        // que repinta, `sync_rows`, etc.) mientras dura el arrastre.
        //
        // Por eso NO podemos llamar `start_drag` aquí adentro: este callback lo invoca el
        // event loop de Slint dentro de un frame, y si `start_drag` entra a su bucle modal
        // con CUALQUIER `RefCell` del controlador prestado (o lo pide un re-entry), revienta
        // con «already borrowed». La causa raíz del crash al arrastrar entre paneles.
        //
        // Fix: capturar las rutas con un `borrow()` CORTO que termina YA, y diferir
        // `start_drag` al próximo turno del event loop con `invoke_from_event_loop`. Cuando
        // ese turno corre, garantizadamente no hay ningún borrow de `ctrl` vivo (el frame que
        // disparó este callback ya cerró), así el bucle modal de `DoDragDrop` no choca con
        // nadie. El cinturón de seguridad complementario está en el tick del timer, que ahora
        // usa `try_borrow_mut` y se salta el tick si el controlador está prestado.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_row_drag_out(move |id| {
            // El origen del arrastre es el panel donde NACIÓ el gesto (`id`), NO el activo: así
            // arrastrar desde un panel inactivo mueve/copia sus archivos sin obligar a un clic
            // previo para activarlo. Borrow corto: clonar las rutas y soltar el préstamo.
            // DEFENSA DE ENTRADA: el .slint hace `row-clicked` justo antes de este callback al
            // arrastrar una fila no seleccionada; si un modificador quedó pegado (Shift/Ctrl), ese
            // row-clicked ya envenenó la selección con un rango fantasma → el drag arrastraría lo
            // incorrecto. No podemos deshacer ese row-clicked, pero limpiar aquí evita que el
            // estado sucio se propague y deja la selección coherente para el siguiente gesto.
            ctrl.borrow_mut().clear_modifiers();
            let paths = ctrl.borrow().selected_paths_of(PaneId(id as u64));
            if paths.is_empty() {
                return;
            }
            crate::logging::breadcrumb(&format!(
                "drag_out: iniciar arrastre ({} ítems)",
                paths.len()
            ));
            // Diferir el arrastre fuera de este frame. `paths` se mueve al closure; ya no hay
            // ningún borrow de `ctrl` en juego cuando `DoDragDrop` arranca su bucle modal.
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                // SEGURIDAD DE DATOS (captura fantasma): marcar `drag-in-progress` ANTES de entrar
                // al bucle modal de `DoDragDrop`. Mientras es true, cada panel Files DESMONTA su
                // TouchArea exterior, así el panel ORIGEN no queda con la captura de puntero
                // huérfana que el bucle modal de Windows nunca le devuelve (el `pointer-up` no llega
                // al TouchArea). Sin esto, tras el arrastre el cursor sobre OTRO panel seguiría
                // resaltando/seleccionando la fila del origen a su misma altura.
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_drag_in_progress(true);
                }
                // `start_drag` es BLOQUEANTE: corre el bucle modal de DoDragDrop hasta que el
                // usuario suelta. Al volver, el gesto terminó (drop dentro/fuera o cancelado).
                let _outcome = naygo_platform::dnd::start_drag(&paths);
                if let Some(ui) = ui_weak.upgrade() {
                    // Re-montar los TouchArea: el gesto del body-touch del origen renace LIMPIO (sin
                    // `pressed-here` ni grab). Un drop intra-app entre paneles llega aparte por
                    // `drop_rx` en el tick del timer; este flag solo controla el montaje del TouchArea.
                    ui.set_drag_in_progress(false);
                    // CRÍTICO: señalar que el drag terminó para que el tick limpie los modificadores.
                    // Durante el bucle modal de DoDragDrop la app NO recibe eventos de teclado, así
                    // que el `key-release` de Shift/Ctrl NUNCA llega a `on_key_release` y quedan
                    // PEGADOS en true. Eso envenena el siguiente `row-clicked` (selección por rango
                    // fantasma al iniciar otro drag o al clicar en otro panel). El controlador no es
                    // accesible aquí (Rc no es Send en `invoke_from_event_loop`), así que lo marca un
                    // flag y el tick del timer (hilo de UI, con acceso a ctrl) hace `clear_modifiers`.
                    ui.set_drag_just_ended(true);
                }
            });
        });
    }
    // Rubber-band (6F): selección por rectángulo arrastrando desde una fila no seleccionada.
    // Ctrl (estado del controlador) hace la selección aditiva.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_rubber_band(move |id, from, to| {
            let additive = ctrl.borrow().ctrl_down;
            ctrl.borrow_mut()
                .select_rect_range(PaneId(id as u64), from, to, additive);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_sort_by(move |id, col| {
            ctrl.borrow_mut()
                .on_sort_by(PaneId(id as u64), col.as_str());
            sync_rows();
        });
    }
    // Ordenar por columna dinámica (header de columnas) (6C).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_sort_by_kind(move |id, kind| {
            ctrl.borrow_mut().sort_by_kind(PaneId(id as u64), kind);
            sync_rows();
        });
    }
    // Mostrar/ocultar una columna (menú "Columnas…") (6C).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_column_toggle(move |id, kind| {
            ctrl.borrow_mut().column_toggle(PaneId(id as u64), kind);
            sync_rows();
        });
    }
    // Reordenar columnas (arrastrar el header) (6C).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_column_move(move |id, from, to| {
            ctrl.borrow_mut().column_move(PaneId(id as u64), from, to);
            sync_rows();
        });
    }
    // Redimensionar una columna (arrastrar su borde) (6C).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_column_resize(move |id, kind, w| {
            ctrl.borrow_mut().column_resize(PaneId(id as u64), kind, w);
            sync_rows();
        });
    }
    // --- Menú/editor de columna (clic derecho en el header, F2) ---
    // Abrir el menú en (x,y) para la columna `kind` del panel `id`.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_column_context(move |id, kind, x, y| {
            ctrl.borrow_mut()
                .column_menu_open(PaneId(id as u64), kind, x, y);
            sync_rows();
        });
    }
    // Ordenar ascendente desde el menú.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_sort_asc(move || {
            ctrl.borrow_mut().column_menu_sort(true);
            sync_rows();
        });
    }
    // Ordenar descendente desde el menú.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_sort_desc(move || {
            ctrl.borrow_mut().column_menu_sort(false);
            sync_rows();
        });
    }
    // Pasar el menú a modo editor de filtro.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_open_filter(move || {
            ctrl.borrow_mut().column_menu_to_filter();
            sync_rows();
        });
    }
    // Quitar el filtro de la columna del menú.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_clear_filter(move || {
            ctrl.borrow_mut().column_menu_clear_filter();
            sync_rows();
        });
    }
    // Ocultar la columna del menú.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_hide(move || {
            ctrl.borrow_mut().column_menu_hide();
            sync_rows();
        });
    }
    // Mover la columna una posición a la izquierda.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_move_left(move || {
            ctrl.borrow_mut().column_menu_move(-1);
            sync_rows();
        });
    }
    // Mover la columna una posición a la derecha.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_move_right(move || {
            ctrl.borrow_mut().column_menu_move(1);
            sync_rows();
        });
    }
    // Editor de filtro: borrador de texto.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_set_text(move |t| {
            ctrl.borrow_mut().column_filter_set_text(t.as_str());
            sync_rows();
        });
    }
    // Editor de filtro: alternar sensibilidad a mayúsculas.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_toggle_case(move || {
            ctrl.borrow_mut().column_filter_toggle_case();
            sync_rows();
        });
    }
    // Editor de filtro: borrador del extremo mínimo (rango).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_set_min(move |t| {
            ctrl.borrow_mut().column_filter_set_range(false, t.as_str());
            sync_rows();
        });
    }
    // Editor de filtro: borrador del extremo máximo (rango).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_set_max(move |t| {
            ctrl.borrow_mut().column_filter_set_range(true, t.as_str());
            sync_rows();
        });
    }
    // Editor de filtro: marcar/desmarcar una extensión.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_toggle_ext(move |e| {
            ctrl.borrow_mut().column_filter_toggle_ext(e.as_str());
            sync_rows();
        });
    }
    // Editor de filtro: aplicar (cierra el menú; la vista se refiltra sola).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_apply(move || {
            ctrl.borrow_mut().column_filter_apply();
            sync_rows();
        });
    }
    // Cerrar el menú (clic fuera).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_dismiss(move || {
            ctrl.borrow_mut().column_menu_close();
            sync_rows();
        });
    }
}
