// Naygo — comprobación de rutas en workers acotados; nunca espera desde la UI.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use naygo_core::CancellationToken;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(5);
static ACTIVE: std::sync::OnceLock<Arc<AtomicUsize>> = std::sync::OnceLock::new();

#[derive(Debug, PartialEq, Eq)]
pub enum ValidationError {
    Invalid,
    Busy,
    Timeout,
}

struct Permit {
    active: Arc<AtomicUsize>,
    _metrics: crate::metrics::JobLease,
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct PathValidation {
    rx: mpsc::Receiver<Result<PathBuf, ValidationError>>,
    began: Instant,
    cancel: CancellationToken,
}

impl Drop for PathValidation {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl PathValidation {
    /// El propietario de UI conserva esta señal aparte del callback del Timer: Slint
    /// retiene el callback al hacer stop(), por lo que no basta con esperar a Drop.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    pub fn start(path: PathBuf) -> Result<Self, ValidationError> {
        Self::spawn(
            ACTIVE.get_or_init(|| Arc::new(AtomicUsize::new(0))).clone(),
            move |cancel| {
                // Resolver . / .. sin convertir todas las rutas a prefijos verbatim ni
                // sustituir el alias/junction elegido por su destino físico. La comprobación
                // posterior sí consulta el SO, siempre aquí. Si desaparece, el listado informa.
                let resolved = std::path::absolute(path).map_err(|_| ValidationError::Invalid)?;
                if !cancel.is_cancelled() && resolved.is_dir() {
                    Ok(resolved)
                } else {
                    Err(ValidationError::Invalid)
                }
            },
        )
    }

    fn spawn(
        active: Arc<AtomicUsize>,
        probe: impl FnOnce(CancellationToken) -> Result<PathBuf, ValidationError> + Send + 'static,
    ) -> Result<Self, ValidationError> {
        crate::metrics::job_submitted(crate::metrics::JobKind::PathValidation);
        active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| {
                crate::metrics::job_rejected(crate::metrics::JobKind::PathValidation);
                ValidationError::Busy
            })?;
        let permit = Permit {
            active,
            _metrics: crate::metrics::job_started(crate::metrics::JobKind::PathValidation),
        };
        let (tx, rx) = mpsc::sync_channel(1);
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        // Adquirir ANTES de crear el hilo; incluye workers cancelados que siguen en el SO.
        std::thread::Builder::new()
            .name("path-validation".into())
            .spawn(move || {
                let _permit = permit;
                if !worker_cancel.is_cancelled() {
                    let result = probe(worker_cancel.clone());
                    if !worker_cancel.is_cancelled() {
                        let _ = tx.send(result);
                    }
                }
            })
            .map_err(|_| ValidationError::Busy)?;
        Ok(Self {
            rx,
            began: Instant::now(),
            cancel,
        })
    }

    /// El timeout es lógico. El permiso sigue en el worker hasta que termine realmente;
    /// cancelar/dropear este receptor no permite crear hilos sin límite en una UNC caída.
    pub fn poll(&self, now: Instant) -> Option<Result<PathBuf, ValidationError>> {
        if now.duration_since(self.began) >= TIMEOUT {
            self.cancel.cancel();
            return Some(Err(ValidationError::Timeout));
        }
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(ValidationError::Invalid)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_and_timeout_do_not_release_running_slots() {
        let active = Arc::new(AtomicUsize::new(0));
        let mut releases = Vec::new();
        for _ in 0..2 {
            let (tx, rx) = mpsc::channel::<()>();
            let (started_tx, started_rx) = mpsc::channel();
            releases.push(tx);
            let job = PathValidation::spawn(active.clone(), move |_| {
                started_tx.send(()).unwrap();
                let _ = rx.recv();
                Ok(PathBuf::from("ok"))
            })
            .unwrap();
            started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            // Equivale a Esc mientras Slint conserva el callback y, con él, el job.
            job.cancellation_token().cancel();
            assert!(job.cancel.is_cancelled());
            assert_eq!(
                job.poll(job.began + TIMEOUT),
                Some(Err(ValidationError::Timeout))
            );
            drop(job);
        }
        assert!(matches!(
            PathValidation::spawn(active.clone(), |_| Ok(PathBuf::new())),
            Err(ValidationError::Busy)
        ));
        drop(releases);
        let until = Instant::now() + Duration::from_secs(3);
        while active.load(Ordering::Acquire) != 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn validates_directories_but_not_files_or_missing_paths() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("file"), b"test").unwrap();
        for (path, valid) in [
            (temp.path().to_path_buf(), true),
            (temp.path().join("file"), false),
            (temp.path().join("absent"), false),
        ] {
            let job = PathValidation::start(path).unwrap();
            let until = Instant::now() + Duration::from_secs(4);
            let result = loop {
                if let Some(result) = job.poll(Instant::now()) {
                    break result;
                }
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(2));
            };
            assert_eq!(result.is_ok(), valid);
        }
    }
}
