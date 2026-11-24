// Naygo — watchers de carpeta por panel (Fase 5A): vigilan la carpeta abierta de cada panel
// Files y aplican los cambios sin re-listar, resaltando los archivos nuevos un tiempo.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Bajo consumo: el watcher de cada carpeta corre en su propio hilo (crate notify, vía
// platform::dir_watch) y está dormido salvo cuando hay un cambio real, que despierta la UI con
// el `waker`. El hilo de UI nunca hace I/O: solo drena el canal en el tick.

use naygo_core::fs_model::Entry;
use naygo_core::listing::DirEvent;
use naygo_platform::dir_watch::{self, Waker};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

/// Un lote de eventos del watcher con la metadata YA resuelta en el hilo relay: la UI aplica
/// los cambios sin tocar el disco. Antes el tick hacía un `fs::metadata` por archivo cambiado,
/// así que un lote grande (Dropbox sincronizando) convertía el tick en una ráfaga de syscalls
/// bloqueantes.
pub struct WatchBatch {
    generation: u64,
    pub events: Vec<DirEvent>,
    /// Entry pre-construido por ruta de evento que lo necesita (Created/Modified/Renamed→to);
    /// `None` = la ruta ya no existe al momento de resolver. Las rutas de Removed/Renamed→from
    /// no se resuelven (no hace falta).
    pub resolved: HashMap<PathBuf, Option<Entry>>,
}

/// Cerrar o navegar solo señala cancelación: no espera I/O ni el destructor nativo.
struct RelayHandle {
    canceled: Arc<AtomicBool>,
    wake: Sender<Vec<DirEvent>>,
}

impl Drop for RelayHandle {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
        let _ = self.wake.send(Vec::new());
    }
}

fn forward_batch(
    tx: &Sender<(u64, WatchBatch)>,
    pane: u64,
    generation: u64,
    events: Vec<DirEvent>,
    waker: &Waker,
) -> bool {
    let batch = WatchBatch {
        generation,
        resolved: resolve_entries(&events),
        events,
    };
    if tx.send((pane, batch)).is_err() {
        return false;
    }
    // Esta es la cola FINAL: cuando la UI despierta ya puede consumir la metadata.
    waker();
    true
}

/// Resuelve los `Entry` de las rutas de `events` que los necesitan (Created / Modified /
/// Renamed→to), haciendo el I/O de metadata. Corre en el hilo relay del watcher, NUNCA en el
/// de UI. Pura y testeable.
fn resolve_entries(events: &[DirEvent]) -> HashMap<PathBuf, Option<Entry>> {
    let mut out = HashMap::new();
    for ev in events {
        let p = match ev {
            DirEvent::Created(p) | DirEvent::Modified(p) => Some(p),
            DirEvent::Renamed { to, .. } => Some(to),
            DirEvent::Removed(_) => None,
        };
        if let Some(p) = p {
            // Metadata del propio enlace (no del destino), igual que el listado.
            out.entry(p.clone())
                .or_insert_with(|| naygo_core::listing::entry_at(p));
        }
    }
    out
}

/// Vigila las carpetas abiertas (una por panel) y junta los eventos en un canal común,
/// etiquetados por PaneId. Cada panel guarda su `WatchHandle` (Drop = deja de vigilar). Las
/// rutas recién aparecidas se recuerdan con su instante para pintarlas resaltadas un tiempo.
pub struct Watchers {
    handles: HashMap<u64, RelayHandle>, // clave: PaneId.0
    tx: Sender<(u64, WatchBatch)>,
    rx: Receiver<(u64, WatchBatch)>,
    /// Rutas resaltadas (recién aparecidas) con el instante de aparición, por panel.
    fresh: HashMap<u64, Vec<(PathBuf, Instant)>>,
    /// La carpeta que vigila cada panel (para no re-vigilar la misma y detectar cambios).
    watched_dir: HashMap<u64, PathBuf>,
    generations: HashMap<u64, u64>,
    next_generation: u64,
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
            generations: HashMap::new(),
            next_generation: 0,
        }
    }

    /// (Re)empieza a vigilar `dir` para el panel `pane`. Reemplaza el watcher anterior (su Drop
    /// lo detiene). `waker` despierta la UI dormida al llegar un evento.
    pub fn watch(&mut self, pane: u64, dir: PathBuf, waker: Waker) {
        // Adaptar el canal de dir_watch (Vec<DirEvent>) a nuestro canal etiquetado por panel:
        // un hilo liviano reenvía cada lote agregándole el PaneId. El MISMO hilo resuelve la
        // metadata de cada ruta (`resolve_entries`), así el tick de UI no hace syscalls.
        let (pane_tx, pane_rx) = channel::<Vec<DirEvent>>();
        self.next_generation = self.next_generation.wrapping_add(1);
        let generation = self.next_generation;
        self.generations.insert(pane, generation);
        // Despertar DESPUÉS de resolver metadata y publicar en el canal que lee la UI.
        // El wake del productor original se adelantaba al relay y podía perderse en reposo.
        let canceled = Arc::new(AtomicBool::new(false));
        self.handles.insert(
            pane,
            RelayHandle {
                canceled: canceled.clone(),
                wake: pane_tx.clone(),
            },
        );
        self.watched_dir.insert(pane, dir.clone());
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            // Tanto registrar la carpeta como liberar el watcher pueden bloquear en red.
            let _watch = dir_watch::watch(&dir, pane_tx, Arc::new(|| {}));
            // Registro completado: reconciliar la ventana entre snapshot y watcher listo.
            // Un lote vacío es una invalidación, no un evento de archivo inventado.
            if canceled.load(Ordering::Acquire)
                || !forward_batch(&tx, pane, generation, Vec::new(), &waker)
            {
                return;
            }
            while let Ok(events) = pane_rx.recv() {
                if canceled.load(Ordering::Acquire)
                    || !forward_batch(&tx, pane, generation, events, &waker)
                {
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
        self.generations.remove(&pane);
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

    /// Drena los eventos pendientes: devuelve (pane, lote con metadata resuelta) acumulados
    /// sin bloquear.
    pub fn drain(&mut self) -> Vec<(u64, WatchBatch)> {
        let mut out = Vec::new();
        while let Ok(item) = self.rx.try_recv() {
            // Un relay antiguo puede terminar después de navegar o cerrar el panel.
            if self.generations.get(&item.0) == Some(&item.1.generation) {
                out.push(item);
            }
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

    /// Rutas del panel `pane` con resaltado vigente (puede repetir una ruta marcada dos veces).
    /// Es O(k) con k = rutas frescas del panel; no recorre el listado.
    pub fn fresh_paths(&self, pane: u64, secs: u64, now: Instant) -> Vec<&Path> {
        if secs == 0 {
            return Vec::new();
        }
        self.fresh
            .get(&pane)
            .map(|v| {
                v.iter()
                    .filter(|(_, t)| now.duration_since(*t).as_secs() < secs)
                    .map(|(p, _)| p.as_path())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Firma del CONJUNTO de rutas frescas vigentes del panel. Solo cambia cuando empieza o vence
    /// un resaltado, así que `rows_signature` puede cachear mientras dura (antes devolvía `None`
    /// y cada tick reconstruía todas las filas). Independiente del orden y de duplicados.
    pub fn fresh_signature(&self, pane: u64, secs: u64, now: Instant) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut paths = self.fresh_paths(pane, secs, now);
        if paths.is_empty() {
            return 0;
        }
        paths.sort_unstable();
        paths.dedup();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        paths.len().hash(&mut h);
        for p in paths {
            p.hash(&mut h);
        }
        h.finish() | 1
    }

    /// Tiempo hasta que vence el próximo resaltado vigente (de cualquier panel). Permite que el
    /// tick duerma hasta ese instante en vez de seguir a 30 ms solo para apagar el resaltado.
    pub fn next_fresh_expiry(&self, secs: u64, now: Instant) -> Option<std::time::Duration> {
        if secs == 0 {
            return None;
        }
        // `is_fresh_ro` usa segundos enteros: vence cuando `elapsed.as_secs() >= secs`, es decir
        // exactamente en `t + secs`.
        let life = std::time::Duration::from_secs(secs);
        self.fresh
            .values()
            .flatten()
            .filter(|(_, t)| now.duration_since(*t).as_secs() < secs)
            .map(|(_, t)| (*t + life).saturating_duration_since(now))
            .min()
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
    fn wake_observes_final_batch_with_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("new.txt");
        std::fs::write(&file, b"ready").unwrap();
        let (tx, rx) = channel();
        let rx = std::sync::Mutex::new(rx);
        let waker: Waker = Arc::new(move || {
            let (pane, batch): (u64, WatchBatch) = rx
                .lock()
                .unwrap()
                .try_recv()
                .expect("wake must follow enqueue");
            assert_eq!(pane, 7);
            assert_eq!(batch.generation, 3);
            assert_eq!(
                batch
                    .resolved
                    .values()
                    .next()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .size,
                Some(5)
            );
        });
        assert!(forward_batch(
            &tx,
            7,
            3,
            vec![DirEvent::Created(file)],
            &waker
        ));
    }

    #[test]
    fn old_generation_and_closed_pane_are_discarded() {
        let mut watchers = Watchers::new();
        watchers.generations.insert(7, 2);
        for (pane, generation) in [(7, 1), (7, 2), (8, 2)] {
            watchers
                .tx
                .send((
                    pane,
                    WatchBatch {
                        generation,
                        events: vec![],
                        resolved: HashMap::new(),
                    },
                ))
                .unwrap();
        }
        let batches = watchers.drain();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].1.generation, 2);
        watchers.unwatch(7);
        watchers
            .tx
            .send((
                7,
                WatchBatch {
                    generation: 2,
                    events: vec![],
                    resolved: HashMap::new(),
                },
            ))
            .unwrap();
        assert!(watchers.drain().is_empty());
    }

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

    #[test]
    fn firma_fresca_solo_cambia_al_empezar_o_vencer() {
        let mut w = Watchers::new();
        let t0 = Instant::now();
        assert_eq!(w.fresh_signature(1, 8, t0), 0);
        w.mark_fresh(1, vec![PathBuf::from("C:/a.txt")], t0);
        let s1 = w.fresh_signature(1, 8, t0);
        assert_ne!(s1, 0);
        // Mientras el conjunto vigente no cambia, la firma es estable (no depende de `now`).
        assert_eq!(
            w.fresh_signature(1, 8, t0 + Duration::from_millis(2500)),
            s1
        );
        // Marcar dos veces la misma ruta no cambia el conjunto.
        w.mark_fresh(
            1,
            vec![PathBuf::from("C:/a.txt")],
            t0 + Duration::from_secs(1),
        );
        assert_eq!(w.fresh_signature(1, 8, t0 + Duration::from_secs(1)), s1);
        let t2 = t0 + Duration::from_secs(2);
        w.mark_fresh(1, vec![PathBuf::from("C:/b.txt")], t2);
        let s2 = w.fresh_signature(1, 8, t2);
        assert_ne!(s2, s1);
        // Otro panel no afecta.
        assert_eq!(w.fresh_signature(2, 8, t2), 0);
        // Vence a.txt (las dos marcas): queda solo b.txt.
        let t9 = t0 + Duration::from_secs(9);
        assert_eq!(w.fresh_paths(1, 8, t9), vec![Path::new("C:/b.txt")]);
        assert_ne!(w.fresh_signature(1, 8, t9), s2);
        assert_eq!(w.fresh_signature(1, 0, t2), 0, "sin resaltado configurado");
    }

    #[test]
    fn proximo_vencimiento_es_el_mas_cercano() {
        let mut w = Watchers::new();
        let t0 = Instant::now();
        assert_eq!(w.next_fresh_expiry(8, t0), None);
        w.mark_fresh(1, vec![PathBuf::from("C:/a.txt")], t0);
        w.mark_fresh(
            2,
            vec![PathBuf::from("C:/b.txt")],
            t0 + Duration::from_secs(3),
        );
        let now = t0 + Duration::from_secs(1);
        assert_eq!(w.next_fresh_expiry(8, now), Some(Duration::from_secs(7)));
        // Vencido a.txt, el próximo es b.txt.
        let now = t0 + Duration::from_secs(8);
        assert_eq!(w.next_fresh_expiry(8, now), Some(Duration::from_secs(3)));
        assert_eq!(w.next_fresh_expiry(8, t0 + Duration::from_secs(20)), None);
        assert_eq!(w.next_fresh_expiry(0, t0), None);
    }

    /// El watcher lee la metadata del ENLACE (no de su destino), igual que el listado: un
    /// junction a un destino caído se resuelve como carpeta en vez de desaparecer.
    #[cfg(windows)]
    #[test]
    fn resolve_entries_no_sigue_enlaces() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !made {
            return;
        }
        std::fs::remove_dir(&target).unwrap();
        let r = resolve_entries(&[DirEvent::Modified(link.clone())]);
        let e = r
            .get(&link)
            .and_then(|o| o.as_ref())
            .expect("el enlace se resuelve sin seguirlo");
        assert_eq!(e.kind, naygo_core::fs_model::EntryKind::Directory);
    }

    #[test]
    fn resolve_entries_resuelve_lo_que_lo_necesita() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"hola").unwrap();
        let old = tmp.path().join("viejo.txt");
        let new = tmp.path().join("nuevo.txt");
        std::fs::write(&new, b"x").unwrap();
        let fantasma = tmp.path().join("borrado.txt");
        let events = vec![
            DirEvent::Created(file.clone()),
            DirEvent::Modified(file.clone()),
            DirEvent::Renamed {
                from: old.clone(),
                to: new.clone(),
            },
            DirEvent::Removed(fantasma.clone()),
        ];
        let r = resolve_entries(&events);
        // Created/Modified del archivo real: Entry con su tamaño (4 bytes).
        let e = r.get(&file).and_then(|o| o.as_ref()).expect("resuelto");
        assert_eq!(e.size, Some(4));
        // Renamed→to se resuelve; Renamed→from y Removed NO (no hace falta para aplicarlos).
        assert!(r.get(&new).and_then(|o| o.as_ref()).is_some());
        assert!(!r.contains_key(&old), "el origen del rename no se lee");
        assert!(!r.contains_key(&fantasma), "un Removed no se lee");
        // Ruta inexistente (Created de algo que ya desapareció): Some(None), tolerante.
        let events = vec![DirEvent::Created(fantasma.clone())];
        let r = resolve_entries(&events);
        assert!(matches!(r.get(&fantasma), Some(None)));
    }
}
