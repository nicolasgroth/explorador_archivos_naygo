// Naygo — ejecutor de acciones Shell fuera del hilo de UI.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! ShellExecute y compañía pueden bloquearse segundos (handler lento, share de red caído),
//! así que el hilo de UI solo encola una orden y vuelve. Un único hilo worker las ejecuta en
//! orden (con COM en STA, ver `naygo_platform::open::shell_job_runner`) y devuelve el
//! resultado por un canal, despertando la UI con el waker. Una orden bloqueada retrasa las
//! siguientes (no se acumulan hilos), pero la interfaz sigue fluida.

use naygo_platform::dir_watch::Waker;
use naygo_platform::open::{ShellError, ShellJob};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Resultado de una orden. `report == false` significa que el error no se muestra al usuario
/// (comportamiento previo de los botones que ignoraban el resultado).
pub struct ShellOutcome {
    pub report: bool,
    pub result: Result<(), ShellError>,
}

pub struct ShellExecutor {
    tx: Sender<(ShellJob, bool)>,
    rx: Receiver<ShellOutcome>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl ShellExecutor {
    /// Arranca el worker. `make_runner` se invoca DENTRO del hilo worker (ahí se inicializa
    /// COM) y devuelve la función que ejecuta cada orden; inyectable para tests sin Win32.
    pub fn spawn<M, R>(make_runner: M) -> Self
    where
        M: FnOnce() -> R + Send + 'static,
        R: FnMut(ShellJob) -> Result<(), ShellError> + 'static,
    {
        let (tx, jobs) = mpsc::channel::<(ShellJob, bool)>();
        let (results, rx) = mpsc::channel::<ShellOutcome>();
        let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
        let waker_for_worker = waker.clone();
        let started = std::thread::Builder::new()
            .name("naygo-shell".into())
            .spawn(move || {
                let mut runner = make_runner();
                while let Ok((job, report)) = jobs.recv() {
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runner(job)))
                            .unwrap_or_else(|_| {
                                Err(ShellError::Failed("Shell worker failed".into()))
                            });
                    if results.send(ShellOutcome { report, result }).is_err() {
                        break;
                    }
                    let waker = waker_for_worker.lock().ok().and_then(|w| w.clone());
                    if let Some(waker) = waker {
                        waker();
                    }
                }
            });
        if let Err(error) = started {
            crate::logging::log_line(&format!("no se pudo crear el hilo Shell: {error}"));
        }
        Self { tx, rx, waker }
    }

    /// Ejecutor real de producción.
    pub fn spawn_system() -> Self {
        Self::spawn(naygo_platform::open::shell_job_runner)
    }

    pub fn set_waker(&self, waker: Waker) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(waker);
        }
    }

    /// Encola la orden y retorna de inmediato (nunca bloquea). Si el worker no existe, el
    /// error vuelve como resultado en el siguiente `drain`.
    pub fn submit(&self, job: ShellJob, report: bool) -> Result<(), ShellError> {
        self.tx
            .send((job, report))
            .map_err(|_| ShellError::Failed("Shell worker disconnected".into()))
    }

    /// Recoge, sin bloquear, los resultados ya disponibles.
    pub fn drain(&self) -> Vec<ShellOutcome> {
        self.rx.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[test]
    fn slow_opener_does_not_block_submit_and_result_arrives_later() {
        let exec = ShellExecutor::spawn(|| {
            |_job: ShellJob| {
                std::thread::sleep(Duration::from_millis(400));
                Err(ShellError::Failed("lento".into()))
            }
        });
        let woke = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let woke2 = woke.clone();
        exec.set_waker(Arc::new(move || {
            woke2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        let start = Instant::now();
        exec.submit(ShellJob::Open(PathBuf::from("x.txt")), true)
            .unwrap();
        assert!(start.elapsed() < Duration::from_millis(50));
        assert!(exec.drain().is_empty());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let got = exec.drain();
            if let Some(outcome) = got.first() {
                assert!(outcome.report);
                assert_eq!(outcome.result.as_ref().unwrap_err().to_string(), "lento");
                break;
            }
            assert!(Instant::now() < deadline, "el resultado nunca llegó");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(woke.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn panicking_opener_is_reported_as_error() {
        let exec =
            ShellExecutor::spawn(|| |_job: ShellJob| -> Result<(), ShellError> { panic!("x") });
        exec.submit(ShellJob::Open(PathBuf::from("x")), false)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(outcome) = exec.drain().pop() {
                assert!(!outcome.report);
                assert!(outcome.result.is_err());
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
