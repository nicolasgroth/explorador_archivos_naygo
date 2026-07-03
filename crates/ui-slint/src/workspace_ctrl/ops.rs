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

    /// Escribe el archivo pegado en `path`, o —si `Settings.paste_confirm` está activo— abre el
    /// modal de confirmación de nombre (NameInput con purpose Paste) con el nombre propuesto
    /// editable; al confirmar, `name_confirm` escribe los `bytes` con el nombre elegido.
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
            std::fs::write(path, &bytes).is_ok()
        }
    }

    /// Recibe rutas externas soltadas (drag&drop OLE, Fase 5D) sobre el panel `dest`: copia
    /// (o mueve si `move_`) a su carpeta, reusando el engine de operaciones de F3 (con sus
    /// diálogos de conflicto, panel de progreso y cancelación). No-op si el panel no es Files o
    /// no hay rutas. Devuelve true si arrancó la operación.
    pub fn drop_external(
        &mut self,
        dest: PaneId,
        sources: Vec<std::path::PathBuf>,
        move_hint: bool,
        copy_forced: bool,
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
        self.ops.start_op(req, label, true);
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
    pub fn drop_at(
        &mut self,
        content_x: f32,
        content_y: f32,
        move_hint: bool,
        copy_forced: bool,
        paths: Vec<std::path::PathBuf>,
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
        // El destino debe ser un panel Files con carpeta resoluble.
        let Some(dest_dir) = self
            .ws
            .pane(target)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            crate::logging::breadcrumb("drop_at: el panel destino no es Files, no-op");
            return false;
        };
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
            self.ws.set_active(pd.dest_pane);
            if let Some(dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
                self.sync_trees_active(dir);
            }
        }
        let req = naygo_core::ops::transfer(pd.is_move, pd.paths, pd.dest_dir);
        self.ensure_ops_pane();
        self.ops.start_op(req, label, true);
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
        {
            let n = self.selected_paths().len();
            let modo = if permanent { "permanente" } else { "papelera" };
            crate::logging::breadcrumb(&format!("eliminar {} ítem(s) ({})", n, modo));
        }
        let paths = self.selected_paths();
        if !paths.is_empty() {
            self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
                sources: paths,
                permanent,
            });
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

    /// Renombrar el ítem enfocado: abre el modal de nombre con el nombre actual.
    /// Rename inline (F2 / menú): pide a la UI abrir el editor en la celda Name de la fila
    /// enfocada del panel Files activo, con la etapa 0 del ciclo (nombre sin extensión). En vez
    /// del modal, marca `rename_requested`; la UI lo consume con `take_rename_request`. (6D)
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
        self.rename_requested = Some((id, pos, 0));
    }

    /// La UI consume el pedido de rename inline (pane, posición de vista, etapa del ciclo F2).
    pub fn take_rename_request(&mut self) -> Option<(PaneId, usize, u8)> {
        self.rename_requested.take()
    }

    /// Nombre actual de la fila en la posición de vista `pos` del panel `id` (para precargar el
    /// editor inline). Vacío si no existe.
    pub fn rename_name_at(&self, id: PaneId, pos: usize) -> String {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.view_entry_at(pos))
            .map(|e| e.name.clone())
            .unwrap_or_default()
    }

    /// Confirma el rename inline de la fila `pos` del panel `id` al nombre `new_name`. Arma la
    /// op de rename (reusa el engine de F3, con su validación) y la lanza. Devuelve true si
    /// arrancó algo (nombre válido y distinto del actual).
    pub fn rename_commit(&mut self, id: PaneId, pos: usize, new_name: &str) -> bool {
        let new_name = new_name.trim();
        let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
            return false;
        };
        let Some(e) = f.view_entry_at(pos) else {
            return false;
        };
        // Sin cambio o nombre inválido → no hacer nada (evita una op vacía o un error del engine).
        if new_name.is_empty()
            || new_name == e.name
            || !naygo_core::ops::names::is_valid_name(new_name)
        {
            return false;
        }
        crate::logging::breadcrumb("renombrar");
        let source = e.path.clone();
        let req = naygo_core::ops::rename(source, new_name.to_string());
        let label = self.config.t("op.rename");
        self.ops.start_op(req, label, true);
        true
    }

    /// Rename EN CADENA: confirma el rename actual y pide abrir el editor en la fila anterior
    /// (`dir < 0`) o siguiente (`dir > 0`), seleccionando el nombre sin extensión (etapa 0,
    /// decisión de Nicolás). Devuelve la nueva posición si la hay (clamp a la vista). (6D)
    pub fn rename_chain(
        &mut self,
        id: PaneId,
        pos: usize,
        new_name: &str,
        dir: i32,
    ) -> Option<usize> {
        self.rename_commit(id, pos, new_name);
        let count = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.view_len())?;
        if count == 0 {
            return None;
        }
        let next = (pos as i32 + dir).clamp(0, count as i32 - 1) as usize;
        // Mover el foco/selección a la fila nueva, para que el scroll la acompañe.
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.select_single(next);
        }
        self.rename_requested = Some((id, next, 0));
        Some(next)
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
        let reqs = naygo_core::ops::undo::to_requests(&self.ops.undo_history[idx].actions);
        self.ops.undo_history[idx].undone = true;
        let label = self.config.t("undo.button");
        for req in reqs {
            self.ops.start_op(req, label.clone(), false);
        }
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
        let reqs = naygo_core::ops::undo::to_requests(&self.ops.undo_history[idx].actions);
        self.ops.undo_history[idx].undone = true;
        let label = self.config.t("undo.button");
        for req in reqs {
            self.ops.start_op(req, label.clone(), false);
        }
        true
    }
}
