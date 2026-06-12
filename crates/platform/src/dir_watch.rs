// Naygo — vigilar una carpeta (crate notify) y emitir DirEvent coalescidos, aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Envuelve `notify` (vía debouncer) para observar UNA carpeta (no recursivo) y emitir
//! lotes de `naygo_core::listing::DirEvent` coalescidos (~300 ms). Tolerante: si no se
//! puede observar (red, permiso), el handle queda inerte (no crashea). El Drop del handle
//! detiene el watcher.

use naygo_core::listing::DirEvent;
use std::path::Path;
use std::sync::mpsc::Sender;

/// Ventana de coalescing del debouncer. Cambios dentro de esta ventana se agrupan
/// en un solo lote antes de emitirse por el canal.
const DEBOUNCE_MS: u64 = 300;

/// Handle de un watcher activo. Al dropearse, detiene el watcher (libera el handle del SO).
pub struct WatchHandle {
    // El debouncer posee el watcher + su hilo; dropearlo los detiene. `Option` para el
    // caso inerte (no se pudo observar). Se guarda como `Box<dyn Any + Send>` para no tener
    // que nombrar el tipo genérico del debouncer aquí (cambia entre versiones de notify).
    _debouncer: Option<Box<dyn std::any::Any + Send>>,
}

/// Empieza a vigilar `dir` (no recursivo). Emite lotes de `DirEvent` por `tx` con
/// coalescing ~300 ms. Si falla (red caída, permiso denegado, ruta inexistente),
/// devuelve un handle inerte (sin crashear): simplemente no llegarán eventos.
/// Waker para despertar la UI tras enviar eventos: la UI normalmente está DORMIDA (no
/// repinta en reposo, clave para el bajo consumo en VMs sin GPU). El watcher corre en su
/// propio hilo, así que necesita un `Fn() + Send + Sync` (típicamente
/// `egui::Context::request_repaint`) para sacar a la UI del sueño cuando hay un evento
/// real. `platform` no depende de egui: recibe el waker como trait object.
pub type Waker = std::sync::Arc<dyn Fn() + Send + Sync>;

pub fn watch(dir: &Path, tx: Sender<Vec<DirEvent>>, waker: Waker) -> WatchHandle {
    match try_watch(dir, tx, waker) {
        Some(deb) => WatchHandle {
            _debouncer: Some(deb),
        },
        None => {
            tracing::warn!(dir = %dir.display(), "no se pudo vigilar la carpeta; handle inerte");
            WatchHandle { _debouncer: None }
        }
    }
}

/// Intenta crear el debouncer y registrar la carpeta. Devuelve el debouncer boxeado
/// (vivo) o `None` si algo falló. No propaga errores: el contrato es tolerante.
fn try_watch(
    dir: &Path,
    tx: Sender<Vec<DirEvent>>,
    waker: Waker,
) -> Option<Box<dyn std::any::Any + Send>> {
    use notify::RecursiveMode;
    use notify_debouncer_full::new_debouncer;
    use std::time::Duration;

    let mut deb = new_debouncer(
        Duration::from_millis(DEBOUNCE_MS),
        None,
        move |res: notify_debouncer_full::DebounceEventResult| {
            // `Ok` trae los eventos coalescidos del lote; `Err` trae errores del backend.
            // Los errores se ignoran (filesystem hostil): el watcher sigue vivo.
            if let Ok(events) = res {
                let mut out = Vec::new();
                for ev in events {
                    // `DebouncedEvent` deref-ea a `notify::Event`; usamos su campo `.event`.
                    normalize_into(&ev.event, &mut out);
                }
                if !out.is_empty() {
                    // Si el receptor se cayó (la UI cambió de carpeta), ignorar el envío.
                    let _ = tx.send(out);
                    // Despertar la UI: estaba dormida, debe drenar y aplicar los eventos.
                    waker();
                }
            }
        },
    )
    .ok()?;

    deb.watch(dir, RecursiveMode::NonRecursive).ok()?;
    Some(Box::new(deb))
}

/// Normaliza un `notify::Event` a uno o más `DirEvent`.
fn normalize_into(ev: &notify::Event, out: &mut Vec<DirEvent>) {
    use notify::EventKind;
    match &ev.kind {
        EventKind::Create(_) => {
            for p in &ev.paths {
                out.push(DirEvent::Created(p.clone()));
            }
        }
        EventKind::Remove(_) => {
            for p in &ev.paths {
                out.push(DirEvent::Removed(p.clone()));
            }
        }
        EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
            // Un rename "completo" trae dos rutas (origen y destino). Si solo viene una
            // (rename parcial: notify a veces emite from/to por separado), la tratamos como
            // creación; `apply_dir_events` reconcilia los huérfanos.
            if ev.paths.len() == 2 {
                out.push(DirEvent::Renamed {
                    from: ev.paths[0].clone(),
                    to: ev.paths[1].clone(),
                });
            } else {
                for p in &ev.paths {
                    out.push(DirEvent::Created(p.clone()));
                }
            }
        }
        EventKind::Modify(_) => {
            for p in &ev.paths {
                out.push(DirEvent::Modified(p.clone()));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind, RenameMode};
    use notify::Event;
    use std::path::PathBuf;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    fn ev(kind: EventKind, paths: &[&str]) -> Event {
        Event {
            kind,
            paths: paths.iter().map(PathBuf::from).collect(),
            attrs: Default::default(),
        }
    }

    #[test]
    fn normaliza_create() {
        let mut out = Vec::new();
        normalize_into(
            &ev(EventKind::Create(CreateKind::File), &["D:/x/a.txt"]),
            &mut out,
        );
        assert_eq!(out, vec![DirEvent::Created(PathBuf::from("D:/x/a.txt"))]);
    }

    #[test]
    fn normaliza_remove() {
        let mut out = Vec::new();
        normalize_into(
            &ev(EventKind::Remove(RemoveKind::File), &["D:/x/a.txt"]),
            &mut out,
        );
        assert_eq!(out, vec![DirEvent::Removed(PathBuf::from("D:/x/a.txt"))]);
    }

    #[test]
    fn normaliza_rename_dos_rutas() {
        let mut out = Vec::new();
        normalize_into(
            &ev(
                EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                &["D:/x/old.txt", "D:/x/new.txt"],
            ),
            &mut out,
        );
        assert_eq!(
            out,
            vec![DirEvent::Renamed {
                from: PathBuf::from("D:/x/old.txt"),
                to: PathBuf::from("D:/x/new.txt"),
            }]
        );
    }

    #[test]
    fn normaliza_rename_una_ruta_es_created() {
        let mut out = Vec::new();
        normalize_into(
            &ev(
                EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                &["D:/x/new.txt"],
            ),
            &mut out,
        );
        assert_eq!(out, vec![DirEvent::Created(PathBuf::from("D:/x/new.txt"))]);
    }

    #[test]
    fn normaliza_modify_data() {
        let mut out = Vec::new();
        normalize_into(
            &ev(
                EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
                &["D:/x/a.txt"],
            ),
            &mut out,
        );
        assert_eq!(out, vec![DirEvent::Modified(PathBuf::from("D:/x/a.txt"))]);
    }

    /// Waker no-op para los tests (no hay UI que despertar).
    fn noop_waker() -> Waker {
        std::sync::Arc::new(|| {})
    }

    #[test]
    fn ruta_inexistente_handle_inerte_no_crashea() {
        let (tx, _rx) = channel();
        // No debe entrar en pánico: handle inerte.
        let _h = watch(Path::new("Z:/ruta/que/no/existe/naygo"), tx, noop_waker());
    }

    /// Smoke test real con el filesystem. `#[ignore]` por ser sensible a timing (depende
    /// del backend del SO y de la ventana de debounce). Correr con:
    /// `cargo test -p naygo-platform dir_watch_smoke -- --ignored --test-threads=1`
    #[test]
    #[ignore]
    fn dir_watch_smoke() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = channel();
        let _h = watch(dir.path(), tx, noop_waker());

        // Dar un instante al watcher para que registre la carpeta antes de tocarla.
        std::thread::sleep(Duration::from_millis(200));
        let archivo = dir.path().join("nuevo.txt");
        std::fs::write(&archivo, b"hola").unwrap();

        // Drenar lotes hasta encontrar el Created del archivo (o timeout).
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut visto = false;
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(batch) => {
                    if batch
                        .iter()
                        .any(|e| matches!(e, DirEvent::Created(p) if p == &archivo))
                    {
                        visto = true;
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        assert!(visto, "se esperaba un DirEvent::Created para {:?}", archivo);
    }
}
