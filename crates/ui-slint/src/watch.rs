// Naygo — watchers de carpeta por panel (Fase 5A): vigilan la carpeta abierta de cada panel
// Files y aplican los cambios sin re-listar, resaltando los archivos nuevos un tiempo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Bajo consumo: el watcher de cada carpeta corre en su propio hilo (crate notify, vía
// platform::dir_watch) y está dormido salvo cuando hay un cambio real, que despierta la UI con
// el `waker`. El hilo de UI nunca hace I/O: solo drena el canal en el tick.

use naygo_core::listing::DirEvent;
use naygo_platform::dir_watch::{self, Waker, WatchHandle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;

/// Vigila las carpetas abiertas (una por panel) y junta los eventos en un canal común,
/// etiquetados por PaneId. Cada panel guarda su `WatchHandle` (Drop = deja de vigilar). Las
/// rutas recién aparecidas se recuerdan con su instante para pintarlas resaltadas un tiempo.
pub struct Watchers {
    handles: HashMap<u64, WatchHandle>, // clave: PaneId.0
    tx: Sender<(u64, Vec<DirEvent>)>,
    rx: Receiver<(u64, Vec<DirEvent>)>,
    /// Rutas resaltadas (recién aparecidas) con el instante de aparición, por panel.
    fresh: HashMap<u64, Vec<(PathBuf, Instant)>>,
    /// La carpeta que vigila cada panel (para no re-vigilar la misma y detectar cambios).
    watched_dir: HashMap<u64, PathBuf>,
}

impl Watchers {
    pub fn new() -> Watchers {
        let (tx, rx) = channel();
        Watchers {
            handles: HashMap::new(),
            tx,
            rx,
            fresh: HashMap::new(),
            watched_dir: HashMap::new(),
        }
    }

    /// (Re)empieza a vigilar `dir` para el panel `pane`. Reemplaza el watcher anterior (su Drop
    /// lo detiene). `waker` despierta la UI dormida al llegar un evento.
    pub fn watch(&mut self, pane: u64, dir: PathBuf, waker: Waker) {
        // Adaptar el canal de dir_watch (Vec<DirEvent>) a nuestro canal etiquetado por panel:
        // un hilo liviano reenvía cada lote agregándole el PaneId.
        let (pane_tx, pane_rx) = channel::<Vec<DirEvent>>();
        let h = dir_watch::watch(&dir, pane_tx, waker);
        self.handles.insert(pane, h);
        self.watched_dir.insert(pane, dir);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            while let Ok(batch) = pane_rx.recv() {
                if tx.send((pane, batch)).is_err() {
                    break;
                }
            }
        });
    }

    /// Deja de vigilar el panel `pane` (Drop del handle detiene el watcher).
    pub fn unwatch(&mut self, pane: u64) {
        self.handles.remove(&pane);
        self.fresh.remove(&pane);
        self.watched_dir.remove(&pane);
    }

    /// La carpeta que está vigilando el panel `pane` ahora mismo (para detectar cambios de
    /// carpeta y re-vigilar). `None` si no lo vigila.
    pub fn current_dir(&self, pane: u64) -> Option<&Path> {
        self.watched_dir.get(&pane).map(|p| p.as_path())
    }

    /// Los paneles que tienen un watcher activo (para podar los que ya no existen).
    pub fn watched_panes(&self) -> Vec<u64> {
        self.handles.keys().copied().collect()
    }

    /// Drena los eventos pendientes: devuelve (pane, eventos) acumulados sin bloquear.
    pub fn drain(&mut self) -> Vec<(u64, Vec<DirEvent>)> {
        let mut out = Vec::new();
        while let Ok(item) = self.rx.try_recv() {
            out.push(item);
        }
        out
    }

    /// Registra rutas nuevas para resaltar en el panel `pane`.
    pub fn mark_fresh(&mut self, pane: u64, paths: Vec<PathBuf>, now: Instant) {
        if paths.is_empty() {
            return;
        }
        let v = self.fresh.entry(pane).or_default();
        for p in paths {
            v.push((p, now));
        }
    }

    /// ¿La ruta está resaltada (apareció hace menos de `secs`)? Solo lectura (no limpia): la
    /// usa `rows_of` que es `&self`. La limpieza de vencidas la hace el tick vía `prune`.
    pub fn is_fresh_ro(&self, pane: u64, path: &Path, secs: u64, now: Instant) -> bool {
        self.fresh
            .get(&pane)
            .map(|v| {
                v.iter()
                    .any(|(p, t)| p == path && now.duration_since(*t).as_secs() < secs)
            })
            .unwrap_or(false)
    }

    /// Limpia las rutas resaltadas vencidas de todos los paneles (la llama el tick).
    pub fn prune(&mut self, secs: u64, now: Instant) {
        for v in self.fresh.values_mut() {
            v.retain(|(_, t)| now.duration_since(*t).as_secs() < secs);
        }
    }

    /// ¿Hay alguna ruta resaltada todavía vigente? (para mantener el timer vivo mientras dure
    /// el resaltado, y que la UI lo apague al vencer).
    pub fn any_fresh(&self, secs: u64, now: Instant) -> bool {
        self.fresh.values().any(|v| {
            v.iter()
                .any(|(_, t)| now.duration_since(*t).as_secs() < secs)
        })
    }
}

impl Default for Watchers {
    fn default() -> Self {
        Watchers::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fresh_caduca_por_tiempo() {
        let mut w = Watchers::new();
        let t0 = Instant::now();
        w.mark_fresh(1, vec![PathBuf::from("C:/a.txt")], t0);
        assert!(w.is_fresh_ro(1, Path::new("C:/a.txt"), 3, t0));
        let later = t0 + Duration::from_secs(5);
        assert!(!w.is_fresh_ro(1, Path::new("C:/a.txt"), 3, later));
        // prune limpia las vencidas.
        w.prune(3, later);
        assert!(!w.any_fresh(3, later));
    }
}
