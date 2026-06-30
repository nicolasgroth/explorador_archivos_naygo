// Naygo — WorkspaceCtrl: sesión, persistencia, watchers y primera ejecución.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
    /// Propaga el modo de operaciones de Settings (cola/paralelo) al controlador de ops. Antes
    /// el motor de cola existía pero arrancaba SIEMPRE en paralelo (el campo de Settings no se
    /// aplicaba), así que la cola era inalcanzable. Se llama al arrancar y al cambiarlo en config.
    pub fn sync_ops_mode(&mut self) {
        use naygo_core::config::OpsMode as CfgMode;
        self.ops.ops_mode = match self.config.settings.ops_mode {
            CfgMode::Queue => crate::ops_ctrl::OpsMode::Queue,
            CfgMode::Parallel => crate::ops_ctrl::OpsMode::Parallel,
        };
    }

    /// Registra una visita en el historial respetando el límite configurado
    /// (`Settings.recent_limit`, clampeado a 1..=100). Centraliza el límite.
    pub(super) fn push_recent(&mut self, dir: std::path::PathBuf) {
        let limit = self.config.settings.recent_limit.clamp(1, 100);
        self.recents.push(dir, limit);
    }

    /// Fija el límite de carpetas recientes, persiste y trunca la lista al nuevo tope.
    /// Cableado desde main.rs: on_recent_limit_changed en la ventana de configuración.
    pub fn set_recent_limit(&mut self, n: usize) {
        self.config.settings.recent_limit = n.clamp(1, 100);
        self.config.save();
        let limit = self.config.settings.recent_limit;
        self.recents.truncate_to(limit);
    }

    /// Estado persistible del workspace (para guardar al cerrar la ventana): disposición,
    /// panel activo, estado de cada panel Files y el tipo de cada panel.
    pub fn session_persist(&self) -> naygo_core::config::WorkspacePersist {
        naygo_core::config::WorkspacePersist {
            version: 1,
            layout: self.ws.layout.clone(),
            active: self.ws.active_id(),
            files: self
                .ws
                .panes()
                .iter()
                .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.to_persist())))
                .collect(),
            purposes: self.ws.panes().iter().map(|p| (p.id, p.purpose)).collect(),
        }
    }

    /// Guarda la sesión actual en disco (la llama la UI al cerrar la ventana).
    pub fn save_session(&self) {
        naygo_core::config::save_workspace(&self.config.config_dir, &self.session_persist());
    }

    /// Intenta restaurar la sesión guardada (al arrancar). Si hay un workspace.json válido,
    /// reemplaza el workspace de arranque por el restaurado y relanza el listado de cada
    /// panel Files + el árbol de cada panel Tree. Si no hay sesión guardada (o el layout es
    /// vacío/corrupto), no hace nada y se conserva el arranque por defecto.
    ///
    /// Devuelve `true` si restauró una sesión, `false` si no había ninguna (primera
    /// ejecución). El llamador usa ese flag para, en primera ejecución, aplicar la
    /// disposición clásica en vez del panel único de arranque.
    pub fn load_session(&mut self) -> bool {
        let (persist, _recovered) =
            naygo_core::config::load_workspace_flagged(&self.config.config_dir);
        let Some(persist) = persist else {
            return false;
        };
        let Some(restored) = Workspace::from_persist(&persist) else {
            return false;
        };
        // Cancelar los listados del arranque por defecto antes de reemplazar el workspace.
        for l in self.listings.values() {
            l.cancel();
        }
        self.listings.clear();
        self.trees.clear();
        // Soltar el deep job si lo había: el workspace va a ser reemplazado completo.
        if let Some(d) = self.deep_job.take() {
            d.token.cancel();
        }
        self.ws = restored;
        // Relanzar el contenido de cada panel restaurado.
        let panes: Vec<(PaneId, PanePurpose, Option<PathBuf>)> = self
            .ws
            .panes()
            .iter()
            .map(|p| {
                (
                    p.id,
                    p.purpose,
                    p.files.as_ref().map(|f| f.current_dir.clone()),
                )
            })
            .collect();
        for (id, purpose, dir) in panes {
            match purpose {
                PanePurpose::Files => {
                    if let Some(dir) = dir {
                        self.push_recent(dir.clone());
                        self.start_listing(id, dir);
                    }
                }
                PanePurpose::Tree => {
                    let mut t = build_tree();
                    if let Some(cur) = self.ws.active_files().map(|f| f.current_dir.clone()) {
                        t.set_active(cur);
                    }
                    self.trees.insert(id, t);
                }
                _ => {}
            }
        }
        // La sesión recién cargada ES el estado en disco: sembrar la huella para no
        // reescribir el mismo workspace.json en el primer tick.
        self.last_saved_fingerprint = Some(self.session_fingerprint());
        // Sembrar el último Files activo con el activo restaurado (o el primer Files).
        self.last_active_files = self
            .ws
            .active_id()
            .filter(|a| self.ws.pane(*a).map(|p| p.purpose) == Some(PanePurpose::Files))
            .or_else(|| self.ws.files_panes().first().copied());
        // Arrancar el REVEAL del árbol hasta la carpeta activa restaurada: antes solo se hacía
        // `set_active` en cada árbol (fijaba el destino) pero no se sembraba `reveal_targets` ni
        // se arrancaba `pump_reveal`, así que al iniciar el árbol no expandía hasta la carpeta.
        if let Some(active_dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
            self.sync_trees_active(active_dir);
        }
        true
    }

    /// Aplica la disposición CLÁSICA de primera ejecución: árbol + dos paneles de
    /// archivos + Propiedades (Inspector) + Vista previa (Preview). Reemplaza el
    /// workspace de arranque (un solo panel) y relanza el contenido de cada panel,
    /// reusando la misma maquinaria que `apply_template`. La llama `main` SOLO cuando
    /// no hay sesión guardada (primera ejecución); las sesiones guardadas se respetan.
    /// A diferencia de `apply_template`, NO registra un uso en la lista de plantillas
    /// recientes (no es una elección del usuario en el menú).
    pub fn apply_first_run_layout(&mut self) {
        let tpl = naygo_core::workspace::LayoutTemplate::primera_ejecucion();
        crate::logging::breadcrumb("aplicar layout clásico (primera ejecución)");
        let home = self.template_home();
        // Cancelar los listados/árboles del arranque por defecto antes de reemplazar el workspace.
        for l in self.listings.values() {
            l.cancel();
        }
        self.listings.clear();
        self.trees.clear();
        self.tree_listings.clear();
        self.reveal_targets.clear();
        if let Some(d) = self.deep_job.take() {
            d.token.cancel();
        }
        self.ws = naygo_core::workspace::Workspace::from_template(&tpl, &home);
        self.relaunch_all_panes();
        self.last_active_files = self.ws.files_panes().first().copied();
    }

    /// Huella barata del estado persistible (lo que cambia entre sesiones que vale la pena
    /// guardar): por cada panel, su id + tipo + carpeta; más la disposición y el activo. NO
    /// incluye scroll ni selección (no se persisten). Si cambia, hay que volver a guardar.
    pub fn session_fingerprint(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        for p in self.ws.panes() {
            let dir = p
                .files
                .as_ref()
                .map(|f| f.current_dir.to_string_lossy().into_owned())
                .unwrap_or_default();
            let _ = write!(s, "{}:{:?}:{}|", p.id.0, p.purpose, dir);
        }
        let _ = write!(
            s,
            "active={:?};layout={:?}",
            self.ws.active_id().map(|a| a.0),
            self.ws
                .layout
                .pane_ids()
                .iter()
                .map(|i| i.0)
                .collect::<Vec<_>>()
        );
        s
    }

    /// Persiste la sesión SOLO si cambió respecto de la última guardada (compara huellas).
    /// La UI la llama en cada tick: barato cuando no hay cambios (solo construye la huella),
    /// y garantiza que la sesión en disco quede al día tras agregar/cerrar/navegar paneles,
    /// sin depender del evento de cierre de ventana.
    pub fn maybe_persist_session(&mut self) {
        let fp = self.session_fingerprint();
        if self.last_saved_fingerprint.as_deref() != Some(fp.as_str()) {
            self.save_session();
            self.last_saved_fingerprint = Some(fp);
        }
    }

    /// Asegura que cada panel Files tenga un watcher sobre su carpeta actual: arranca uno si
    /// falta o si la carpeta cambió, y quita los watchers de paneles que ya no existen. La UI lo
    /// llama en el tick (tiene el `waker` de Slint). Es barato cuando nada cambió.
    pub fn reconcile_watchers(&mut self, waker: naygo_platform::dir_watch::Waker) {
        // Paneles Files actuales y su carpeta.
        let files: Vec<(PaneId, std::path::PathBuf)> = self
            .ws
            .panes()
            .iter()
            .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.current_dir.clone())))
            .collect();
        let vivos: std::collections::HashSet<u64> = files.iter().map(|(id, _)| id.0).collect();
        // Quitar watchers de paneles que ya no existen.
        for pane in self.watchers.watched_panes() {
            if !vivos.contains(&pane) {
                self.watchers.unwatch(pane);
            }
        }
        // Arrancar/re-arrancar el watcher de cada panel cuya carpeta cambió.
        for (id, dir) in files {
            let needs = self.watchers.current_dir(id.0) != Some(dir.as_path());
            if needs {
                self.watchers.watch(id.0, dir, waker.clone());
            }
        }
    }

    /// Aplica un lote de eventos del watcher al panel `pane`: muta sus entries SIN re-listar,
    /// re-ordena, resalta+selecciona los archivos recién llegados (Created), y devuelve las rutas
    /// NUEVAS (para que el watcher las marque "fresh"). No-op si el panel no es Files.
    ///
    /// Dos comportamientos, ambos sobre los Created (no se distingue origen app vs externo: tanto
    /// una copia/movida dentro de Naygo como un cambio externo se tratan igual, coherente con el
    /// resaltado ámbar):
    /// - Resaltado + selección: los nuevos quedan SELECCIONADOS por su posición de vista, para que
    ///   el usuario vea cuáles llegaron (sobre todo tras copiar). El foco va al último (scroll).
    /// - "Archivos nuevos al final" (setting `new_items_at_end`): se empuja `group_new_at_end` al
    ///   panel, de modo que `view_indices` agrupe las recién aparecidas al final de la vista en vez
    ///   de en su posición ordenada. Al refrescar (F5) o navegar, el resaltado se limpia y todo
    ///   vuelve a su orden normal.
    pub fn apply_watch_events(
        &mut self,
        pane: PaneId,
        events: &[naygo_core::listing::DirEvent],
    ) -> Vec<std::path::PathBuf> {
        // Espejo runtime del setting "agrupar al final" (se lee antes del préstamo mutable).
        let group_new_at_end = self.config.settings.new_items_at_end;
        // Carpeta que el panel muestra AHORA (se lee antes del préstamo mutable). Sirve para
        // descartar eventos rezagados de la carpeta ANTERIOR (M-2): al navegar A→B (mismo PaneId)
        // el watcher de A se reemplaza, pero eventos de A ya encolados pueden drenarse después de
        // repoblar B; aplicarlos metería una fila fantasma (un archivo de A que no está en B). El
        // filtro es defensivo y cosmético: si no podemos resolver la carpeta, no filtramos nada.
        let current_dir = self
            .ws
            .pane(pane)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone());
        let Some(f) = self.ws.pane_mut(pane).and_then(|p| p.files.as_mut()) else {
            return Vec::new();
        };
        // Quedarse solo con los eventos cuya ruta pertenece a la carpeta actual del panel. Un
        // evento se conserva si ALGUNA de sus rutas cuelga directamente de `current_dir`; así un
        // `Renamed` que entra o sale de la carpeta se sigue tratando (la lógica de
        // `apply_dir_events` ya se autocorrige). Sin `current_dir` (no resoluble), se aplican todos.
        //
        // La comparación de carpeta padre es case-insensitive (Windows lo es) para no descartar por
        // error un evento legítimo si el watcher reporta la ruta con otra capitalización. Como NUNCA
        // canonicalizamos (la carpeta vigilada es la MISMA `PathBuf` que navegó el panel, y `notify`
        // une los nombres sobre ella), en la práctica el padre del evento ya coincide con
        // `current_dir`; este filtro solo descarta los rezagados de OTRA carpeta (M-2).
        let belongs = |p: &std::path::Path| -> bool {
            match (&current_dir, p.parent()) {
                (Some(dir), Some(parent)) => paths_eq_ci(parent, dir),
                // Sin carpeta actual resoluble, o ruta sin padre: no filtrar (conservar el evento).
                _ => true,
            }
        };
        let filtered: Vec<naygo_core::listing::DirEvent> = events
            .iter()
            .filter(|ev| {
                use naygo_core::listing::DirEvent::*;
                match ev {
                    Created(p) | Removed(p) | Modified(p) => belongs(p),
                    Renamed { from, to } => belongs(from) || belongs(to),
                }
            })
            .cloned()
            .collect();
        // `read_entry` debe devolver `Option<Entry>`: leemos metadata (puede fallar si la ruta
        // ya desapareció) y armamos el Entry.
        let nuevas = naygo_core::listing::apply_dir_events(&mut f.entries, &filtered, &|p| {
            std::fs::metadata(p)
                .ok()
                .map(|m| naygo_core::listing::entry_from_path(p, Some(&m)))
        });
        let spec = f.sort;
        naygo_core::sort::sort_entries(&mut f.entries, &spec);
        // Empujar el flag ANTES de calcular posiciones: si está activo, la vista pone los nuevos al
        // final y la selección debe apuntar a esas posiciones finales.
        f.group_new_at_end = group_new_at_end;
        // Resaltar + seleccionar los recién llegados (MEJORA 1). Los que estén ocultos por
        // filtro/visibilidad se resaltan pero no se seleccionan (no están en la vista).
        f.select_arrivals(&nuevas);
        nuevas
    }

    /// Sincroniza el conjunto `highlighted` de cada panel Files con el set autoritativo de rutas
    /// "frescas" del watcher (las que siguen vigentes según `highlight_secs`). Lo llama el tick
    /// tras `prune`: cuando a una ruta se le vence el resaltado, se quita de `highlighted` y, si
    /// estaba "agrupada al final", vuelve a su posición ordenada en la próxima vista. Mantiene una
    /// única fuente de verdad (el watcher) entre el resaltado ámbar y el agrupar-al-final.
    pub fn sync_highlighted_from_watchers(&mut self, highlight_secs: u64, now: std::time::Instant) {
        let WorkspaceCtrl { ws, watchers, .. } = self;
        for pane in ws.panes_mut() {
            let id = pane.id.0;
            if let Some(f) = pane.files.as_mut() {
                f.sync_highlighted(|p| watchers.is_fresh_ro(id, p, highlight_secs, now));
            }
        }
    }

    /// Tras un cambio de unidades (USB enchufado/quitado), reubica a `home` los paneles Files
    /// cuya carpeta ya no existe (p. ej. el USB se sacó). Devuelve los panes reubicados para
    /// que la UI re-liste su contenido. No crashea: "el filesystem es hostil".
    pub fn relocate_orphans(&mut self, _home: &std::path::Path) -> Vec<PaneId> {
        // Ya NO reubica ni abre un popup global: cada panel cuya carpeta desapareció muestra el
        // aviso "carpeta no encontrada" IN-PLACE (lo decide `pane_dir_missing` en el builder del
        // PaneVm). Acá no hay nada que hacer salvo no mandar el panel a HOME en silencio.
        Vec::new()
    }
}

/// ¿`a` y `b` son la MISMA ruta en un FS case-insensitive (Windows)? Compara componente a
/// componente plegando a minúscula ASCII, así no da falsos negativos por capitalización ni por
/// el separador. (Equivalente a `core::ops::plan::paths_eq_ci`, que es privado de ese módulo;
/// se replica aquí para el filtro de eventos del watcher de `apply_watch_events`.)
fn paths_eq_ci(a: &std::path::Path, b: &std::path::Path) -> bool {
    let ca: Vec<_> = a.components().collect();
    let cb: Vec<_> = b.components().collect();
    ca.len() == cb.len()
        && ca.iter().zip(&cb).all(|(x, y)| {
            x.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&y.as_os_str().to_string_lossy())
        })
}
