// Naygo — WorkspaceCtrl: operaciones de archivo (copiar/mover/pegar/borrar/renombrar/deshacer).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
    /// El id del panel Files activo (o el primer Files), para dirigir navegaciones desde
    /// paneles auxiliares (un Tree/Favoritos activo no es un Files).
    pub(super) fn active_files_id(&self) -> Option<PaneId> {
        let active = self.ws.active_id();
        if let Some(a) = active {
            if self.ws.pane(a).map(|p| p.purpose) == Some(PanePurpose::Files) {
                return Some(a);
            }
        }
        // El activo no es un panel Files (p. ej. el Árbol): usar el ÚLTIMO Files activo (el que
        // el usuario venía usando), si todavía existe; si no, el primer Files que haya.
        if let Some(last) = self.last_active_files {
            if self.ws.pane(last).map(|p| p.purpose) == Some(PanePurpose::Files) {
                return Some(last);
            }
        }
        self.ws.files_panes().first().copied()
    }

    /// Carpeta del panel Files activo (destino de pegar/nuevo). None si no hay Files.
    pub fn active_dir(&self) -> Option<PathBuf> {
        self.ws.active_files().map(|f| f.current_dir.clone())
    }

    /// Rutas reales de los ítems SELECCIONADOS del panel Files activo (o, si no hay
    /// selección, el ítem enfocado). Vacío si no hay nada. Para las operaciones de archivo.
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        match self.ws.active_id() {
            Some(id) => self.selected_paths_of(id),
            None => Vec::new(),
        }
    }

    /// Rutas seleccionadas (o la enfocada si no hay selección) del panel `id` CONCRETO, sin
    /// depender de cuál esté activo. Lo usa el arrastre entre paneles: el origen es el panel donde
    /// nació el gesto, no el activo (si no, arrastrar desde un panel inactivo no movía nada y
    /// obligaba a un clic extra para activarlo primero). Vacío si `id` no es un panel Files.
    pub fn selected_paths_of(&self, id: PaneId) -> Vec<PathBuf> {
        if self
            .ws
            .pane(id)
            .is_some_and(|p| p.purpose == PanePurpose::Basket)
        {
            return self.basket_action_paths();
        }
        let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
            return Vec::new();
        };
        let view = f.view_indices();
        let mut out: Vec<PathBuf> = f
            .selected
            .iter()
            .filter_map(|&pos| view.get(pos).and_then(|&real| f.entries.get(real)))
            .map(|e| e.path.clone())
            .collect();
        if out.is_empty() {
            if let Some(e) = f.focused_view_entry() {
                out.push(e.path.clone());
            }
        }
        out
    }

    // --- Gestos de operaciones de archivo (delegan en OpsCtrl) ---

    /// Copiar la selección al portapapeles (limpia el corte).
    pub fn op_copy(&mut self) {
        let paths = self.selected_paths();
        if !paths.is_empty() {
            self.ops.set_copy(&paths);
        }
    }

    /// Duplica la selección en su misma carpeta. La desambiguación y el recorrido del árbol se
    /// resuelven en el worker del core; este controlador no toca disco en el hilo UI.
    pub fn op_duplicate(&mut self) -> bool {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return false;
        }
        let req = naygo_core::ops::OpRequest {
            kind: naygo_core::ops::OpKind::Duplicate,
            sources: paths,
            dest_dir: None,
            conflict: naygo_core::ops::ConflictPolicy::Overwrite,
        };
        self.ensure_ops_pane();
        self.ops
            .start_op(req, self.config.t("action.duplicate"), true);
        true
    }

    /// Cortar la selección (marca corte visual).
    pub fn op_cut(&mut self) {
        let paths = self.selected_paths();
        if !paths.is_empty() {
            self.ops.set_cut(&paths);
        }
    }

    /// Pegar en la carpeta activa los archivos del portapapeles. Devuelve true si arrancó
    /// una operación (para reactivar el timer). El pegado de texto/imagen se cablea con el
    /// modal PastePreview (fase de diálogos); aquí solo archivos.
    /// Pega el contenido del portapapeles en la carpeta activa. Reusa `core::clipboard::
    /// decide_paste`, que decide según el contenido + Settings: archivos → transferencia;
    /// TEXTO → crea un .txt (nombre/extensión configurables); IMAGEN → crea un .png/.jpg
    /// (nombre/formato configurables). Antes solo manejaba archivos (el texto/imagen no hacían
    /// nada). Por ahora escribe directo (sin el modal de confirmación de nombre de egui).
    pub fn op_paste(&mut self) -> bool {
        let Some(dir) = self.active_dir() else {
            return false;
        };
        let content = naygo_platform::clipboard::read();
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let exists = |p: &std::path::Path| p.exists();
        let plan = naygo_core::clipboard::decide_paste(
            &content,
            &dir,
            &self.config.settings,
            now_secs,
            &exists,
        );
        use naygo_core::clipboard::PastePlan;
        match plan {
            PastePlan::Transfer { paths, cut } => {
                if paths.is_empty() {
                    return false;
                }
                let label = if cut {
                    self.config.t("ops.file_kind_move")
                } else {
                    self.config.t("ops.file_kind_copy")
                };
                let req = naygo_core::ops::transfer(cut, paths, dir);
                self.ensure_ops_pane();
                self.ops.start_op(req, label, true);
                self.ops.clear_cut();
                true
            }
            PastePlan::CreateText { path, body } => {
                self.paste_write_or_confirm(&dir, &path, body.into_bytes())
            }
            PastePlan::CreateImage { path, fmt, img } => {
                match naygo_core::clipboard::encode::encode_image(
                    &img,
                    fmt,
                    self.config.settings.paste_jpg_quality,
                ) {
                    Ok(bytes) => self.paste_write_or_confirm(&dir, &path, bytes),
                    Err(_) => false,
                }
            }
            PastePlan::Nothing => false,
        }
    }

    /// Abre el selector del historial interno; sus entradas ya están en memoria.
    pub fn op_paste_history(&mut self) -> bool {
        self.ops.open_clipboard_history()
    }

    /// Pega una entrada del historial mediante el mismo motor cancelable usado por Ctrl+V.
    pub fn paste_history_entry(&mut self, index: usize) -> bool {
        let Some(dir) = self.active_dir() else {
            self.ops.pending_dialog = None;
            return false;
        };
        let Some(entry) = self.ops.take_clipboard_history_entry(index) else {
            return false;
        };
        let label = if entry.cut {
            self.config.t("ops.file_kind_move")
        } else {
            self.config.t("ops.file_kind_copy")
        };
        let req = naygo_core::ops::transfer(entry.cut, entry.paths, dir);
        self.ensure_ops_pane();
        self.ops.start_op(req, label, true);
        if entry.cut {
            self.ops.clear_cut();
        }
        true
    }

    /// Escribe el archivo pegado en `path` (en un hilo worker: el `fs::write` no bloquea la UI
    /// y un fallo se reporta con toast), o —si `Settings.paste_confirm` está activo— abre el
    /// modal de confirmación de nombre (NameInput con purpose Paste) con el nombre propuesto
    /// editable; al confirmar, `name_confirm` escribe los `bytes` con el nombre elegido (también
    /// en worker).
    fn paste_write_or_confirm(
        &mut self,
        dir: &std::path::Path,
        path: &std::path::Path,
        bytes: Vec<u8>,
    ) -> bool {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if self.config.settings.paste_confirm {
            self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::NameInput {
                purpose: crate::ops_ctrl::NamePurpose::Paste { ext, bytes },
                dir: dir.to_path_buf(),
                buf: stem,
            });
            true
        } else {
            // Escritura en un hilo worker (un disco lento/lleno o un share de red no congelan
            // la UI). Si falla, `pump_ops` deja el error para que la UI lo anuncie con un toast
            // (antes `fs::write(...).is_ok()` lo tragaba en silencio).
            self.ops.spawn_paste_write(path.to_path_buf(), bytes);
            true
        }
    }

    /// Recibe rutas externas soltadas (drag&drop OLE, Fase 5D) sobre el panel `dest`: copia
    /// (o mueve si `move_`) a su carpeta, reusando el engine de operaciones de F3 (con sus
    /// diálogos de conflicto, panel de progreso y cancelación). No-op si el panel no es Files o
    /// no hay rutas. Devuelve true si arrancó la operación.
    #[allow(dead_code)]
    pub fn drop_external(
        &mut self,
        dest: PaneId,
        sources: Vec<std::path::PathBuf>,
        move_hint: bool,
        copy_forced: bool,
    ) -> bool {
        self.drop_external_with_staging(dest, sources, move_hint, copy_forced, None)
    }

    /// Variante del drop externo que conserva un staging virtual hasta el terminal de la op.
    pub fn drop_external_with_staging(
        &mut self,
        dest: PaneId,
        sources: Vec<std::path::PathBuf>,
        move_hint: bool,
        copy_forced: bool,
        staging: Option<naygo_platform::drop_target::StagedDrop>,
    ) -> bool {
        if sources.is_empty() {
            return false;
        }
        let Some(dir) = self
            .ws
            .pane(dest)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            return false;
        };
        // Misma decisión que `drop_at`: usar las señales FIABLES del OLE (Shift=`move_hint`,
        // Ctrl=`copy_forced`) + mismo disco, vía `decide_drop`. Antes este fallback decidía solo por
        // `move_` (Shift), así que un Ctrl+arrastre que cayera aquí (drop fuera de un panel Files
        // concreto) movía en vez de copiar en el mismo disco — la misma pérdida de datos que se
        // arregló en `drop_at`. `is_move` alimenta ejecución y label por igual (el label no miente).
        let same = naygo_core::dnd::same_drive(&sources[0], &dir);
        let move_ = matches!(
            naygo_core::dnd::decide_drop(move_hint, copy_forced, same),
            naygo_core::dnd::DropAction::Move
        );
        let label = if move_ {
            self.config.t("ops.file_kind_move")
        } else {
            self.config.t("ops.file_kind_copy")
        };
        // Diagnóstico: este es el camino de FALLBACK (el drop no se pudo enrutar por el punto y
        // cayó al panel activo). Si aquí el destino coincide con el origen, el resultado es un
        // no-op silencioso — clave para diagnosticar drops "que no hacen nada".
        crate::logging::breadcrumb(&format!(
            "drop_external (fallback panel activo): {} {} ítem(s) → {}",
            label,
            sources.len(),
            dir.display(),
        ));
        let req = naygo_core::ops::transfer(move_, sources, dir);
        self.ensure_ops_pane();
        self.ops
            .start_op_with_staging(req, label, true, staging.into_iter().collect());
        true
    }

    /// Recibe un drop OLE en el PUNTO `(content_x, content_y)` (coordenadas de contenido, el
    /// mismo sistema que usa `pane_rects`/`drop_hit`): enruta al panel Files que está BAJO el
    /// cursor, no al panel activo. Decide mover/copiar con `decide_drop`, usando SOLO las señales
    /// fiables del OLE: `move_hint` (Shift real) y `copy_forced` (Ctrl real), ambas leídas del
    /// `grfKeyState` al soltar. Los flags de teclado de la app NO se usan aquí: durante el bucle
    /// modal de `DoDragDrop` llegan stale. Prioridad: Shift→mover, Ctrl→copiar, si no según disco.
    ///
    /// No-op (devuelve false) si: no hay rutas, el punto no cae sobre ningún panel, el panel
    /// destino no es Files, o el destino ES la misma carpeta de origen de las rutas (soltar
    /// sobre la propia carpeta). Devuelve true si arrancó la operación.
    #[allow(dead_code)]
    pub fn drop_at(
        &mut self,
        content_x: f32,
        content_y: f32,
        move_hint: bool,
        copy_forced: bool,
        paths: Vec<std::path::PathBuf>,
    ) -> bool {
        self.drop_at_with_staging(content_x, content_y, move_hint, copy_forced, paths, None)
    }

    /// Variante que enlaza el staging de un origen OLE virtual con el modal/cola/operación.
    pub fn drop_at_with_staging(
        &mut self,
        content_x: f32,
        content_y: f32,
        move_hint: bool,
        copy_forced: bool,
        paths: Vec<std::path::PathBuf>,
        staging: Option<naygo_platform::drop_target::StagedDrop>,
    ) -> bool {
        use naygo_core::dnd::{decide_drop, same_drive, DropAction};
        use naygo_core::workspace::layout::drop_hit;
        if paths.is_empty() {
            crate::logging::breadcrumb("drop_at: sin rutas, no-op");
            return false;
        }
        // Panel bajo el cursor, reusando la maquinaria de hit-testing del docking. `last_area`
        // es el área de contenido que la UI mantiene actualizada con `set_area`.
        let panes = self.pane_rects(self.last_area);
        let hit = drop_hit(&panes, content_x, content_y);
        // Diagnóstico: coords (ya en sistema de contenido), nº de rutas, move_ y a qué panel
        // (índice de orden visual en `panes`) acertó el hit-testing — o None si cayó fuera.
        let hit_idx = hit
            .as_ref()
            .and_then(|(id, _)| panes.iter().position(|(pid, _)| pid == id));
        let hit_idx_str = hit_idx
            .map(|i| i.to_string())
            .unwrap_or_else(|| "None".to_string());
        crate::logging::breadcrumb(&format!(
            "drop_at: content=({:.1},{:.1}) rutas={} move_hint={} → panel={}",
            content_x,
            content_y,
            paths.len(),
            move_hint,
            hit_idx_str,
        ));
        let Some((target, _zone)) = hit else {
            return false;
        };
        // La bandeja temporal es un destino LOGICO: soltar aquí agrega referencias, nunca
        // copia/mueve archivos. Resolverla antes del camino Files también evita que el caller
        // interprete el `false` como "drop no enrutado" y aplique el fallback sobre el panel
        // Files activo (que era la causa de los movimientos fantasma de 0 elementos).
        if self.ws.pane(target).map(|p| p.purpose) == Some(PanePurpose::Basket) {
            let added = self.basket.add(paths);
            if added > 0 {
                if let Some(staging) = staging {
                    self.basket_staging.push(staging);
                }
            }
            crate::logging::breadcrumb(&format!(
                "drop_at: {} ítem(s) agregados a bandeja temporal",
                added
            ));
            return true;
        }

        // El resto de destinos debe ser un panel Files con carpeta resoluble. D-1: si el
        // `body-touch` de Slint detectó que el cursor cayó SOBRE una fila-carpeta de ESTE panel,
        // esa carpeta pasa a ser el destino. Así el arrastre dentro del mismo panel conserva todo
        // el flujo habitual (decisión copiar/mover, confirmación y conflictos) y deja de ser un
        // no-op por comparar contra la carpeta actualmente abierta.
        let Some((panel_dir, row_dest)) =
            self.ws
                .pane(target)
                .and_then(|p| p.files.as_ref())
                .map(|f| {
                    let row_dest = self
                        .drag_over_row
                        .filter(|(pane, _)| *pane == target)
                        .and_then(|(_, row)| f.view_indices().get(row).copied())
                        .and_then(|entry_idx| f.entries.get(entry_idx))
                        .filter(|entry| entry.is_dir())
                        .map(|entry| entry.path.clone());
                    (f.current_dir.clone(), row_dest)
                })
        else {
            crate::logging::breadcrumb("drop_at: el panel destino no es Files, no-op");
            return false;
        };
        let dropped_on_folder = row_dest.is_some();
        let dest_dir = row_dest.unwrap_or(panel_dir);
        if dropped_on_folder {
            crate::logging::breadcrumb(&format!(
                "drop_at: fila-carpeta destino detectada → {}",
                dest_dir.display()
            ));
        }
        // No permitir soltar una carpeta sobre sí misma ni dentro de uno de sus descendientes.
        // `Path::starts_with` compara componentes, no prefijos de texto, y también cubre el caso
        // de la carpeta exacta. Esta defensa es previa al motor: evita requests imposibles y
        // conserva los archivos de origen intactos.
        if paths.iter().any(|source| dest_dir.starts_with(source)) {
            crate::logging::breadcrumb(
                "drop_at: destino dentro del origen (carpeta sobre sí misma), no-op",
            );
            return false;
        }
        // Soltar sobre la propia carpeta de origen es no-op: si todas las rutas ya viven en
        // `dest_dir`, no hay nada que copiar/mover. (Comparar el padre de cada ruta con el
        // destino; basta con que alguna venga de otra carpeta para proceder.)
        let all_from_dest = paths.iter().all(|p| {
            p.parent()
                .map(|par| par == dest_dir.as_path())
                .unwrap_or(false)
        });
        if all_from_dest {
            crate::logging::breadcrumb("drop_at: soltado sobre la propia carpeta, no-op");
            return false;
        }
        // Acción según las señales del OLE + mismo disco. CLAVE: `move_hint` (Shift) y
        // `copy_forced` (Ctrl) vienen del grfKeyState que Windows entrega al SOLTAR, así que
        // reflejan las teclas REALES en ese instante. Los flags de teclado de la app NO sirven
        // aquí: durante el bucle modal de DoDragDrop la app no recibe eventos de teclado y llegan
        // stale (false) aunque el usuario tenga Ctrl/Shift presionado. `decide_drop` prioriza
        // Shift→mover, Ctrl→copiar, si no según disco (mismo→mover, distinto→copiar).
        let same = same_drive(&paths[0], &dest_dir);
        let is_move = matches!(decide_drop(move_hint, copy_forced, same), DropAction::Move);
        let label = if is_move {
            self.config.t("ops.file_kind_move")
        } else {
            self.config.t("ops.file_kind_copy")
        };
        // CONFIRMAR AL SOLTAR (decisión de Nicolás): NO ejecutamos la op aquí. Guardamos el drop ya
        // validado en `pending_drop` y devolvemos true; la UI abre un modal "¿Copiar/Mover N a
        // «destino»?" y, al confirmar, llama a `confirm_pending_drop` que arranca la op de verdad.
        // Así un arrastre accidental entre paneles no copia/mueve archivos sin que el usuario lo vea.
        let count = paths.len();
        crate::logging::breadcrumb(&format!(
            "drop_at: {} {} ítem(s) → {} (pendiente de confirmar)",
            label,
            count,
            dest_dir.display(),
        ));
        self.pending_drop = Some(PendingDrop {
            paths: paths.clone(),
            dest_dir: dest_dir.clone(),
            dest_pane: target,
            is_move,
            count,
            staging,
        });
        // UN SOLO POPUP COHERENTE (decisión de Nicolás): si el drop CHOCA con archivos que ya
        // existen en el destino, NO mostramos primero "¿Copiar/Mover…?" y luego el conflicto —
        // serían dos popups en cadena. En ese caso vamos DIRECTO a la op: el motor abre el diálogo
        // de CONFLICTO (comparación lado a lado), que YA es la confirmación (Saltar/Sobrescribir/
        // Mantener ambos/Cancelar). La confirmación "¿Copiar?" solo aporta cuando NO hay choque.
        let hay_conflicto = {
            let req = naygo_core::ops::transfer(is_move, paths, dest_dir);
            self.ops.first_collision(&req)
        };
        // Confirmación opcional (decisión de Nicolás): el modal "¿Copiar/Mover…?" se muestra SOLO
        // cuando (a) el ajuste está encendido Y (b) NO hay conflicto. Si hay conflicto, o el ajuste
        // está apagado, ejecutamos directo reusando `confirm_pending_drop` (arranca la op y CONSUME
        // `pending_drop`); el modal de CONFLICTO, si aplica, lo dispara `pump_ops` aparte.
        //
        // Devolvemos `true` en AMBOS casos (el drop fue ENRUTADO y manejado por este panel, así la
        // UI no cae al fallback `drop_external`). Quién decide abrir el modal de confirmación es el
        // llamador, que lee `pending_drop` DESPUÉS: queda `Some` → abre el modal; `None` (ya
        // ejecutado directo) → no abre nada (pero el conflicto puede aparecer luego).
        let confirmar = self.config.settings.confirm_drop_between_panes && !hay_conflicto;
        if !confirmar {
            let motivo = if hay_conflicto {
                "hay conflicto → directo al diálogo de conflicto (sin doble popup)"
            } else {
                "confirmación de drop desactivada → ejecutar directo"
            };
            crate::logging::breadcrumb(&format!("drop_at: {motivo}"));
            self.confirm_pending_drop();
        }
        true
    }

    /// El usuario CONFIRMÓ el drop pendiente (botón Copiar/Mover del modal): arranca la op real.
    /// Devuelve true si había un drop pendiente y se lanzó. No-op (false) si no había ninguno.
    pub fn confirm_pending_drop(&mut self) -> bool {
        let Some(pd) = self.pending_drop.take() else {
            return false;
        };
        let label = if pd.is_move {
            self.config.t("ops.file_kind_move")
        } else {
            self.config.t("ops.file_kind_copy")
        };
        crate::logging::breadcrumb(&format!(
            "confirm_pending_drop: {} {} ítem(s) → {}",
            label,
            pd.count,
            pd.dest_dir.display(),
        ));
        // Activar el panel destino: tras soltar, el foco queda donde aterrizaron los archivos (lo
        // más intuitivo para seguir trabajando ahí). Solo si sigue existiendo.
        if self.ws.pane(pd.dest_pane).is_some() {
            self.set_active(pd.dest_pane);
            if let Some(dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
                self.sync_trees_for_files(pd.dest_pane, dir);
            }
        }
        let req = naygo_core::ops::transfer(pd.is_move, pd.paths, pd.dest_dir);
        self.ensure_ops_pane();
        self.ops
            .start_op_with_staging(req, label, true, pd.staging.into_iter().collect());
        true
    }

    /// El usuario CANCELÓ el drop pendiente (botón Cancelar / Esc / clic fuera): lo descarta sin
    /// copiar ni mover nada.
    pub fn cancel_pending_drop(&mut self) {
        if self.pending_drop.take().is_some() {
            crate::logging::breadcrumb("cancel_pending_drop: drop descartado por el usuario");
        }
    }

    /// Eliminar la selección: abre el modal de confirmación.
    pub fn op_delete(&mut self, permanent: bool) {
        // Un menú contextual de breadcrumb/árbol no tiene filas seleccionadas en el FilePanel.
        // En ese caso la fuente de verdad es su target explícito; el atajo Delete conserva la
        // selección normal cuando no hay menú abierto.
        let paths = {
            let contextual = self.context_targets();
            if contextual.is_empty() {
                self.selected_paths()
            } else {
                contextual
            }
        };
        {
            let n = paths.len();
            let modo = if permanent { "permanente" } else { "papelera" };
            crate::logging::breadcrumb(&format!("eliminar {} ítem(s) ({})", n, modo));
        }
        if !paths.is_empty() {
            // Permanente siempre confirma. Papelera respeta la preferencia existente: cuando está
            // desactivada se ejecuta directo; cuando está activa, el modal incluye el preview.
            if !permanent && !self.config.settings.confirm_trash {
                self.ensure_ops_pane();
                let req = naygo_core::ops::delete(paths, true);
                self.ops
                    .start_op(req, self.config.t("ops.file_kind_delete"), true);
            } else {
                self.ensure_ops_pane();
                self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
                    sources: paths,
                    permanent,
                });
            }
        }
    }

    /// Nuevo archivo/carpeta en la carpeta activa: abre el modal de nombre.
    pub fn op_new(&mut self, is_dir: bool) {
        let Some(dir) = self.active_dir() else {
            return;
        };
        // El título del modal y la etiqueta de la op se traducen aquí (el caller tiene `config`);
        // `OpsCtrl` los recibe ya resueltos porque no conoce el idioma.
        let purpose = if is_dir {
            crate::ops_ctrl::NamePurpose::NewDir {
                label: self.config.t("op.new_folder"),
            }
        } else {
            crate::ops_ctrl::NamePurpose::NewFile {
                label: self.config.t("op.new_file"),
            }
        };
        self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::NameInput {
            purpose,
            dir,
            buf: String::new(),
        });
    }

    // --- Comprimir / Extraer (.zip) ---

    /// Abre el modal de nombre para comprimir la selección en un `.zip` en la carpeta del panel
    /// activo. El campo arranca con `default_zip_name` (nombre del único ítem, o "archivos.zip"
    /// para varios). Al confirmar el modal, `name_confirm` arma la op `Compress` y la lanza.
    /// No-op si no hay selección o no hay carpeta activa.
    pub fn op_compress_prompt(&mut self) {
        let sources = self.selected_paths();
        if sources.is_empty() {
            return;
        }
        let Some(dir) = self.active_dir() else {
            return;
        };
        let default_name = naygo_core::archive_ops::default_zip_name(&sources);
        // Asegurar el panel de ops ANTES de mostrar el modal (igual que op_delete/op_extract_to):
        // así, al confirmar el nombre, el panel de progreso ya está visible. Consistencia de UX.
        self.ensure_ops_pane();
        self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::NameInput {
            purpose: crate::ops_ctrl::NamePurpose::Compress { sources },
            dir,
            buf: default_name,
        });
    }

    /// Extrae el `.zip` seleccionado (el primero de la selección) dentro de `dest` (una carpeta).
    /// No-op si no hay selección.
    pub fn op_extract_to(&mut self, dest: std::path::PathBuf) {
        let Some(zip) = self.selected_paths().into_iter().next() else {
            return;
        };
        let req = naygo_core::ops::OpRequest {
            kind: naygo_core::ops::OpKind::Extract,
            sources: vec![zip],
            dest_dir: Some(dest),
            conflict: naygo_core::ops::ConflictPolicy::Ask,
        };
        self.ensure_ops_pane();
        let label = self.config.t("ops.kind_extract");
        self.ops.start_op(req, label, true);
    }

    /// "Extraer aquí": crea (a través del motor) una subcarpeta con el nombre del zip sin
    /// extensión dentro de la carpeta activa y extrae ahí. No-op si no hay selección/carpeta.
    pub fn op_extract_here(&mut self) {
        let Some(zip) = self.selected_paths().into_iter().next() else {
            return;
        };
        let Some(dir) = self.active_dir() else {
            return;
        };
        let sub = zip
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extraido");
        self.op_extract_to(dir.join(sub));
    }

    /// `true` si la selección del panel activo es EXACTAMENTE un archivo `.zip` (para mostrar las
    /// entradas "Extraer aquí" / "Extraer en…" del menú contextual). Compara la extensión sin
    /// importar mayúsculas; una carpeta o varios ítems → false.
    pub fn sel_is_single_zip(&self) -> bool {
        let sel = self.selected_paths();
        if sel.len() != 1 {
            return false;
        }
        let p = &sel[0];
        p.is_file()
            && p.extension()
                .map(|e| e.eq_ignore_ascii_case("zip"))
                .unwrap_or(false)
    }

    /// Rename inline (F2 / menú): pide a la UI abrir el editor en la celda Name de la fila
    /// enfocada. F2 repetido sobre el mismo archivo recorre nombre → extensión → todo.
    pub fn op_rename(&mut self) {
        let Some(id) = self.active_files_id() else {
            return;
        };
        let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
            return;
        };
        let Some(pos) = f.focused else {
            return;
        };
        let Some(entry) = f.view_entry_at(pos) else {
            return;
        };
        let stage = self
            .rename_active
            .as_ref()
            .filter(|r| r.pane == id && r.source == entry.path)
            .map_or(0, |r| (r.stage + 1) % 3);
        self.request_rename_at(id, pos, stage);
    }

    /// La UI consume el pedido de mostrar/recrear el editor. `rename_active` se conserva hasta
    /// confirmar o cancelar para validar callbacks tardíos e identificar el archivo por ruta.
    pub fn take_rename_request(&mut self) -> Option<super::RenameRequest> {
        self.rename_requested.take()
    }

    /// Comprueba que un callback pertenece al editor que sigue activo. La UI lo consulta antes de
    /// cerrar sus propiedades: un callback tardío debe ser un no-op completo y no cerrar la sesión
    /// nueva aunque la operación de filesystem ya esté protegida.
    pub fn rename_session_is_active(&self, id: PaneId, session: u32) -> bool {
        self.rename_active
            .as_ref()
            .is_some_and(|active| active.pane == id && active.session == session)
    }

    /// Crea una sesión de rename anclada a la ruta actual de la fila. Las carpetas se seleccionan
    /// completas aunque contengan puntos; para archivos se aplica el ciclo F2.
    fn request_rename_at(&mut self, id: PaneId, pos: usize, stage: u8) -> bool {
        let Some(entry) = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.view_entry_at(pos))
        else {
            return false;
        };
        let name = entry.name.clone();
        let source = entry.path.clone();
        let selection = if entry.is_dir() {
            (0, name.len())
        } else {
            naygo_core::rename::rename_selection_byte_offsets(&name, stage)
        };
        let session = self.next_rename_session;
        self.next_rename_session = self.next_rename_session.wrapping_add(1).max(1);
        let request = super::RenameRequest {
            session,
            pane: id,
            pos,
            source,
            name,
            stage,
            selection,
        };
        self.rename_active = Some(request.clone());
        self.rename_requested = Some(request);
        true
    }

    /// Confirma una sesión de rename. El id de sesión vuelve idempotentes Enter + pérdida de
    /// foco y evita que un callback tardío del editor anterior afecte al siguiente del chain.
    pub fn rename_commit(&mut self, id: PaneId, session: u32, new_name: &str) -> bool {
        let new_name = new_name.trim();
        let Some(active) = self.rename_active.as_ref() else {
            return false;
        };
        if active.pane != id || active.session != session {
            return false;
        }
        // La sesión se comprobó inmediatamente antes; si el invariante se rompe, no-op.
        let Some(active) = self.rename_active.take() else {
            return false;
        };
        self.rename_requested = None;
        // Sin cambio o nombre inválido → no hacer nada (evita una op vacía o un error del engine).
        if new_name.is_empty()
            || new_name == active.name
            || !naygo_core::ops::names::is_valid_name(new_name)
        {
            return false;
        }
        crate::logging::breadcrumb("renombrar");
        let req = naygo_core::ops::rename(active.source, new_name.to_string());
        let label = self.config.t("op.rename");
        self.ops.start_op(req, label, true);
        true
    }

    /// Cancela la sesión antes de destruir el editor. Así su notificación posterior de pérdida
    /// de foco no puede convertir Esc en una confirmación accidental.
    pub fn rename_cancel(&mut self) {
        self.rename_active = None;
        self.rename_requested = None;
    }

    /// Rename EN CADENA: confirma el rename actual y pide abrir el editor en la fila anterior
    /// (`dir < 0`) o siguiente (`dir > 0`), seleccionando el nombre sin extensión (etapa 0,
    /// decisión de Nicolás). Devuelve la nueva posición si la hay (clamp a la vista). (6D)
    pub fn rename_chain(
        &mut self,
        id: PaneId,
        session: u32,
        new_name: &str,
        dir: i32,
    ) -> Option<usize> {
        let current_pos = {
            let active = self.rename_active.as_ref()?;
            if active.pane != id || active.session != session {
                return None;
            }
            self.ws
                .pane(id)
                .and_then(|p| p.files.as_ref())
                .and_then(|f| {
                    f.view_indices().iter().position(|&real| {
                        f.entries.get(real).is_some_and(|e| e.path == active.source)
                    })
                })
                .unwrap_or(active.pos)
        };
        self.rename_commit(id, session, new_name);
        let count = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.view_len())?;
        if count == 0 {
            return None;
        }
        let next = (current_pos as i32 + dir).clamp(0, count as i32 - 1) as usize;
        // Mover el foco/selección a la fila nueva, para que el scroll la acompañe.
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.select_single(next);
        }
        self.request_rename_at(id, next, 0).then_some(next)
    }

    /// Arma el texto de PREVISUALIZACIÓN del deshacer para el popup de confirmación (antes de
    /// ejecutar). Devuelve `None` si la entrada no existe, ya se deshizo, o el inverso ya no aplica
    /// (`validate`). Todo se compone con `config.t(...)` (i18n) para respetar el idioma activo.
    ///
    /// El resultado `(resumen, lineas)`:
    /// - `resumen`: una frase con el verbo y el conteo, p. ej. "Deshacer «Copiar»: se borrarán 2
    ///   archivo(s)" o "Deshacer «Mover»: se devolverán 3 archivo(s) a su origen". El verbo se deriva
    ///   de las `actions` (predominancia de `TrashCreated` vs `MoveBack`); el conteo es la cantidad
    ///   de acciones.
    /// - `lineas`: una por acción — el nombre del archivo + a dónde va (papelera / carpeta origen).
    pub fn undo_preview(&self, id: u64) -> Option<(String, Vec<String>)> {
        let idx = self.ops.undo_history.iter().position(|e| e.id == id)?;
        let entry = &self.ops.undo_history[idx];
        if entry.undone || naygo_core::ops::undo::validate(&entry.actions).is_err() {
            return None;
        }
        let n = entry.actions.len();
        // ¿Es un deshacer de COPIAR/CREAR (trashea) o de MOVER/RENOMBRAR (devuelve)? Se decide por
        // el tipo predominante de acción: si hay al menos un MoveBack, el verbo es "devolver"; si
        // todas son TrashCreated, es "borrar". (Una entrada mezcla un solo tipo en la práctica, pero
        // el criterio es robusto ante cualquier combinación.)
        let has_move = entry
            .actions
            .iter()
            .any(|a| matches!(a, naygo_core::ops::undo::UndoAction::MoveBack { .. }));
        // Resumen: "Deshacer «<label>»: se {borrarán|devolverán} N archivo(s) [a su origen]".
        let phrase_key = if has_move {
            "slint.undo.will_move_back"
        } else {
            "slint.undo.will_delete"
        };
        let phrase = self.config.t(phrase_key).replace("{n}", &n.to_string());
        let summary = self
            .config
            .t("slint.undo.summary")
            .replace("{label}", &entry.label)
            .replace("{detail}", &phrase);
        // Lista: una línea por acción. TrashCreated → "<nombre> → Papelera"; MoveBack → "<nombre> →
        // <carpeta destino>". Los nombres/carpetas se derivan de las rutas de cada acción.
        let to_trash = self.config.t("slint.undo.to_trash");
        let to_arrow = self.config.t("slint.undo.to_arrow");
        let file_name = |p: &std::path::Path| -> String {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned())
        };
        let folder_of = |p: &std::path::Path| -> String {
            p.parent()
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let lines: Vec<String> = entry
            .actions
            .iter()
            .map(|a| match a {
                naygo_core::ops::undo::UndoAction::TrashCreated { path } => {
                    format!("{} {} {}", file_name(path), to_arrow, to_trash)
                }
                naygo_core::ops::undo::UndoAction::MoveBack { now, back_to } => {
                    format!("{} {} {}", file_name(now), to_arrow, folder_of(back_to))
                }
                naygo_core::ops::undo::UndoAction::RestoreTrash { original, .. } => {
                    format!(
                        "{} {} {}",
                        file_name(original),
                        to_arrow,
                        folder_of(original)
                    )
                }
            })
            .collect();
        Some((summary, lines))
    }

    /// Deshace la entrada del historial con `id` (botón "Deshacer" del panel Historial).
    /// Valida, re-emite el inverso y la marca deshecha. Devuelve true si arrancó algo.
    pub fn undo_entry(&mut self, id: u64) -> bool {
        let Some(idx) = self.ops.undo_history.iter().position(|e| e.id == id) else {
            return false;
        };
        if self.ops.undo_history[idx].undone
            || naygo_core::ops::undo::validate(&self.ops.undo_history[idx].actions).is_err()
        {
            return false;
        }
        let actions = self.ops.undo_history[idx].actions.clone();
        let reqs = naygo_core::ops::undo::to_requests(&actions);
        let receipts = trash_restore_receipts(&actions);
        self.ops.undo_history[idx].undone = true;
        let label = self.config.t("undo.button");
        for req in reqs {
            self.ops.start_op(req, label.clone(), false);
        }
        self.ops.start_trash_restore(receipts, label);
        true
    }

    /// Deshace la última entrada deshacible del historial. Devuelve true si arrancó algo.
    pub fn op_undo_last(&mut self) -> bool {
        // Buscar la última entrada no-deshecha y deshacible.
        let idx = self
            .ops
            .undo_history
            .iter()
            .rposition(|e| !e.undone && naygo_core::ops::undo::validate(&e.actions).is_ok());
        let Some(idx) = idx else {
            return false;
        };
        let actions = self.ops.undo_history[idx].actions.clone();
        let reqs = naygo_core::ops::undo::to_requests(&actions);
        let receipts = trash_restore_receipts(&actions);
        self.ops.undo_history[idx].undone = true;
        let label = self.config.t("undo.button");
        for req in reqs {
            self.ops.start_op(req, label.clone(), false);
        }
        self.ops.start_trash_restore(receipts, label);
        true
    }
}

/// Convierte las acciones de restauración de core a los recibos Shell que la capa
/// platform necesita. Mantenerlo aquí preserva la frontera: core no depende de Windows.
fn trash_restore_receipts(
    actions: &[naygo_core::ops::undo::UndoAction],
) -> Vec<naygo_platform::trash::TrashReceipt> {
    actions
        .iter()
        .filter_map(|action| match action {
            naygo_core::ops::undo::UndoAction::RestoreTrash { original } => {
                Some(naygo_platform::trash::TrashReceipt {
                    original: original.clone(),
                })
            }
            _ => None,
        })
        .collect()
}
