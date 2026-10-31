// Naygo — métricas locales y opt-in de rendimiento, sin datos de archivos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Muestras efímeras de duración para diagnosticar rendimiento por solicitud explícita.
//! `NAYGO_PERF_METRICS=1` activa una cola acotada; el event loop solo usa `try_send` y las
//! líneas de resumen se escriben al log local desde un worker al cerrar la aplicación.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, SyncSender, TrySendError},
        Mutex, OnceLock,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const QUEUE_CAPACITY: usize = 512;
const SAMPLES_PER_KIND: usize = 2_048;
const MIN_SAMPLES_FOR_P95: usize = 100;

static ENABLED: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static JOBS: [JobStats; 3] = [const { JobStats::new() }; 3];

#[derive(Clone, Copy)]
pub(crate) enum JobKind {
    Preview,
    PathValidation,
    Pack,
}

impl JobKind {
    fn index(self) -> usize {
        match self {
            Self::Preview => 0,
            Self::PathValidation => 1,
            Self::Pack => 2,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::PathValidation => "path_validation",
            Self::Pack => "pack",
        }
    }
}

struct JobStats {
    submitted: AtomicU64,
    active: AtomicUsize,
    peak: AtomicUsize,
    started: AtomicU64,
    finished: AtomicU64,
    rejected: AtomicU64,
}

impl JobStats {
    const fn new() -> Self {
        Self {
            submitted: AtomicU64::new(0),
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            started: AtomicU64::new(0),
            finished: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
        }
    }

    fn start(&self) {
        self.started.fetch_add(1, Ordering::Relaxed);
        let active = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        self.peak.fetch_max(active, Ordering::Relaxed);
    }
}

/// Captura el ciclo de vida real del permiso, incluso si el colector se apaga mientras el
/// worker sigue vivo. Sin métricas activas conserva sólo el enum, sin reloj, cola ni bloqueo.
pub(crate) struct JobLease(Option<JobKind>);

impl JobLease {
    fn tracked(kind: JobKind) -> Self {
        JOBS[kind.index()].start();
        Self(Some(kind))
    }
}

impl Drop for JobLease {
    fn drop(&mut self) {
        if let Some(kind) = self.0 {
            let stats = &JOBS[kind.index()];
            stats.active.fetch_sub(1, Ordering::AcqRel);
            stats.finished.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(crate) fn job_started(kind: JobKind) -> JobLease {
    if ENABLED.load(Ordering::Acquire) {
        JobLease::tracked(kind)
    } else {
        JobLease(None)
    }
}

/// Cuenta trabajos enviados incluso cuando se descartan por cola llena o worker ausente.
pub(crate) fn job_submitted(kind: JobKind) {
    if ENABLED.load(Ordering::Acquire) {
        JOBS[kind.index()].submitted.fetch_add(1, Ordering::Relaxed);
    }
}

/// Sólo obtiene reloj si la captura opt-in está activa.
pub(crate) fn queue_timestamp() -> Option<Instant> {
    ENABLED.load(Ordering::Acquire).then(Instant::now)
}

pub(crate) fn job_rejected(kind: JobKind) {
    if ENABLED.load(Ordering::Acquire) {
        JOBS[kind.index()].rejected.fetch_add(1, Ordering::Relaxed);
    }
}

enum Message {
    Sample(Sample),
    Shutdown,
}

#[derive(Clone, Copy)]
struct Sample {
    kind: &'static str,
    micros: u64,
}

struct Runtime {
    sender: SyncSender<Message>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

/// Span barato; si las métricas están apagadas no lee el reloj ni crea eventos.
pub(crate) struct Span {
    kind: &'static str,
    started: Option<Instant>,
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            record(self.kind, started.elapsed());
        }
    }
}

/// Activa métricas solo mediante la variable explícita `NAYGO_PERF_METRICS=1`.
pub(crate) fn init() {
    if std::env::var_os("NAYGO_PERF_METRICS").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    // `init` is called once by main, but keep this safe if a future startup path reuses it.
    // Avoid starting an unattached collector when the OnceLock already has a runtime.
    if RUNTIME.get().is_some() {
        return;
    }
    let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
    let worker = match std::thread::Builder::new()
        .name("naygo-perf-metrics".into())
        .spawn(move || collect(receiver))
    {
        Ok(worker) => worker,
        Err(error) => {
            crate::logging::log_line(&format!("no se inició el worker de métricas: {error}"));
            return;
        }
    };
    if RUNTIME
        .set(Runtime {
            sender,
            worker: Mutex::new(Some(worker)),
        })
        .is_ok()
    {
        ENABLED.store(true, Ordering::Release);
        crate::logging::log_line(
            "métricas locales opt-in activadas; sin rutas ni nombres de archivo",
        );
    }
}

pub(crate) fn span(kind: &'static str) -> Span {
    Span {
        kind,
        started: ENABLED.load(Ordering::Acquire).then(Instant::now),
    }
}

pub(crate) fn record(kind: &'static str, duration: Duration) {
    if !ENABLED.load(Ordering::Acquire) {
        return;
    }
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    match runtime.sender.try_send(Message::Sample(Sample {
        kind,
        micros: duration.as_micros().min(u64::MAX as u128) as u64,
    })) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Detiene el colector y espera el resumen solo después de salir del event loop.
pub(crate) fn shutdown() {
    ENABLED.store(false, Ordering::Release);
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    stop_and_join(&runtime.sender, &runtime.worker);
}

fn stop_and_join(sender: &SyncSender<Message>, worker: &Mutex<Option<JoinHandle<()>>>) {
    // El envío puede esperar si la cola está llena. Este helper sólo se llama después del loop UI.
    let _ = sender.send(Message::Shutdown);
    if let Some(worker) = worker
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take()
    {
        let _ = worker.join();
    }
}

fn collect(receiver: mpsc::Receiver<Message>) {
    collect_into(receiver, crate::logging::log_line);
}

fn collect_into(receiver: mpsc::Receiver<Message>, mut log_line: impl FnMut(&str)) {
    let mut stats = BTreeMap::<&'static str, SampleStats>::new();
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Sample(sample) => stats.entry(sample.kind).or_default().push(sample.micros),
            Message::Shutdown => break,
        }
    }
    for (kind, stats) in stats {
        log_line(&stats.format(kind));
    }
    for kind in [JobKind::Preview, JobKind::PathValidation, JobKind::Pack] {
        let stats = &JOBS[kind.index()];
        log_line(&format!(
            "[perf] jobs={} submitted={} started={} finished={} active={} peak_active={} rejected={}",
            kind.label(),
            stats.submitted.load(Ordering::Relaxed),
            stats.started.load(Ordering::Relaxed),
            stats.finished.load(Ordering::Relaxed),
            stats.active.load(Ordering::Acquire),
            stats.peak.load(Ordering::Relaxed),
            stats.rejected.load(Ordering::Relaxed),
        ));
    }
    let dropped = DROPPED.swap(0, Ordering::Relaxed);
    if dropped > 0 {
        log_line(&format!(
            "[perf] dropped_samples={dropped}; la cola acotada estaba llena o cerrada"
        ));
    }
}

#[derive(Default)]
struct SampleStats {
    count: u64,
    samples: VecDeque<u64>,
}

impl SampleStats {
    fn push(&mut self, micros: u64) {
        self.count = self.count.saturating_add(1);
        if self.samples.len() == SAMPLES_PER_KIND {
            self.samples.pop_front();
        }
        self.samples.push_back(micros);
    }

    fn format(&self, kind: &str) -> String {
        let mut sorted = self.samples.iter().copied().collect::<Vec<_>>();
        sorted.sort_unstable();
        let p50 = percentile(&sorted, 50);
        let p95 = if sorted.len() >= MIN_SAMPLES_FOR_P95 {
            percentile(&sorted, 95).map(|value| format!(" p95_us={value}"))
        } else {
            None
        };
        let p50 = p50.map_or_else(|| "n/a".into(), |value| value.to_string());
        let p95 = p95.unwrap_or_default();
        let max = sorted.last().copied().unwrap_or_default();
        format!(
            "[perf] kind={kind} count={} retained={} p50_us={p50}{p95} max_us={max}",
            self.count,
            sorted.len()
        )
    }
}

fn percentile(sorted: &[u64], percentile: usize) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (sorted.len() * percentile).div_ceil(100).saturating_sub(1);
    sorted.get(rank).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metricas_apagadas_no_consultan_el_reloj_ni_registran() {
        ENABLED.store(false, Ordering::Release);
        assert!(span("test.disabled").started.is_none());
        record("test.disabled", Duration::from_millis(1));
        assert!(RUNTIME.get().is_none());
    }

    #[test]
    fn cola_llena_descarta_sin_bloquear() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        sender
            .try_send(Message::Sample(Sample {
                kind: "test.queue",
                micros: 1,
            }))
            .unwrap();
        assert!(matches!(
            sender.try_send(Message::Sample(Sample {
                kind: "test.queue",
                micros: 2,
            })),
            Err(TrySendError::Full(_))
        ));
    }

    #[test]
    fn percentil_95_se_publica_solo_con_muestras_suficientes() {
        let mut stats = SampleStats::default();
        for sample in 0..MIN_SAMPLES_FOR_P95 - 1 {
            stats.push(sample as u64);
        }
        assert!(!stats.format("test.p95").contains("p95_us="));
        stats.push(99);
        assert!(stats.format("test.p95").contains("p95_us=94"));
    }

    #[test]
    fn muestra_no_tiene_campo_para_ruta_o_nombre() {
        let sample = Sample {
            kind: "listing.poll",
            micros: 42,
        };
        assert_eq!(sample.kind, "listing.poll");
        assert_eq!(sample.micros, 42);
    }

    #[test]
    fn lease_de_worker_refleja_su_ciclo_de_vida() {
        let stats = &JOBS[JobKind::Preview.index()];
        let active_before = stats.active.load(Ordering::Acquire);
        let started_before = stats.started.load(Ordering::Relaxed);
        let finished_before = stats.finished.load(Ordering::Relaxed);
        {
            let _lease = JobLease::tracked(JobKind::Preview);
            assert_eq!(stats.active.load(Ordering::Acquire), active_before + 1);
            assert!(stats.peak.load(Ordering::Relaxed) > active_before);
        }
        assert_eq!(stats.active.load(Ordering::Acquire), active_before);
        assert_eq!(stats.started.load(Ordering::Relaxed), started_before + 1);
        assert_eq!(stats.finished.load(Ordering::Relaxed), finished_before + 1);
    }

    #[test]
    fn cierre_integrado_drena_muestras_antes_de_unir_el_colector() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let lines = std::sync::Arc::new(Mutex::new(Vec::new()));
        let worker_lines = lines.clone();
        let worker = std::thread::spawn(move || {
            collect_into(receiver, |line| {
                worker_lines.lock().unwrap().push(line.to_owned());
            });
        });
        let handle = Mutex::new(Some(worker));
        sender
            .send(Message::Sample(Sample {
                kind: "pack.queue_wait",
                micros: 7,
            }))
            .unwrap();

        stop_and_join(&sender, &handle);

        let lines = lines.lock().unwrap();
        assert!(lines
            .iter()
            .any(|line| line.contains("kind=pack.queue_wait")));
        assert!(handle.lock().unwrap().is_none());
    }
}
