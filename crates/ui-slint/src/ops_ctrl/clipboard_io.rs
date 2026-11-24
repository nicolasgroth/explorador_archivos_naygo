// Naygo — worker serial para las operaciones del portapapeles del sistema.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Serializa I/O del clipboard fuera del hilo UI y preserva el orden entre leer y escribir.

use naygo_core::clipboard::ClipboardContent;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

const QUEUE_CAPACITY: usize = 32;

pub(crate) struct ClipboardIo {
    sender: SyncSender<Request>,
}

#[derive(Clone)]
pub(crate) struct ClipboardIoHandle {
    sender: SyncSender<Request>,
}

pub(crate) enum ClipboardWriteKind {
    Files { paths: Arc<[PathBuf]>, cut: bool },
    Text,
}

pub(crate) struct PendingClipboardWrite {
    pub receiver: Receiver<Result<(), String>>,
    pub kind: ClipboardWriteKind,
}

enum Request {
    Read(mpsc::Sender<ClipboardContent>),
    WriteFiles {
        paths: Arc<[PathBuf]>,
        cut: bool,
        reply: mpsc::Sender<Result<(), String>>,
    },
    WriteText {
        text: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
}

impl ClipboardIo {
    pub fn new() -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("naygo-clipboard-io".into())
            .spawn(move || process_requests(receiver))?;
        Ok(Self { sender })
    }

    pub fn handle(&self) -> ClipboardIoHandle {
        ClipboardIoHandle {
            sender: self.sender.clone(),
        }
    }
}

impl ClipboardIoHandle {
    /// Lectura bloqueante permitida sólo desde workers. Un mensaje de lectura ocupa su posición
    /// en la misma cola que las escrituras, por lo que Ctrl+C/Ctrl+V seguidos conservan el orden.
    pub fn read(&self) -> ClipboardContent {
        let (reply, result) = mpsc::channel();
        if self.sender.send(Request::Read(reply)).is_err() {
            return ClipboardContent::Empty;
        }
        result.recv().unwrap_or(ClipboardContent::Empty)
    }

    /// Encola una lectura para callbacks que no pueden esperar por el sistema operativo.
    pub fn read_async(&self) -> Result<Receiver<ClipboardContent>, String> {
        let (reply, result) = mpsc::channel();
        self.try_send(Request::Read(reply))?;
        Ok(result)
    }

    pub fn write_files(
        &self,
        paths: Vec<PathBuf>,
        cut: bool,
    ) -> Result<PendingClipboardWrite, String> {
        let paths: Arc<[PathBuf]> = paths.into();
        let (reply, result) = mpsc::channel();
        self.try_send(Request::WriteFiles {
            paths: paths.clone(),
            cut,
            reply,
        })?;
        Ok(PendingClipboardWrite {
            receiver: result,
            kind: ClipboardWriteKind::Files { paths, cut },
        })
    }

    pub fn write_text(&self, text: String) -> Result<PendingClipboardWrite, String> {
        let (reply, result) = mpsc::channel();
        self.try_send(Request::WriteText { text, reply })?;
        Ok(PendingClipboardWrite {
            receiver: result,
            kind: ClipboardWriteKind::Text,
        })
    }

    fn try_send(&self, request: Request) -> Result<(), String> {
        self.sender.try_send(request).map_err(|error| match error {
            TrySendError::Full(_) => "cola del portapapeles ocupada".to_string(),
            TrySendError::Disconnected(_) => "worker del portapapeles desconectado".to_string(),
        })
    }
}

fn process_requests(receiver: Receiver<Request>) {
    while let Ok(request) = receiver.recv() {
        match request {
            Request::Read(reply) => {
                let _ = reply.send(naygo_platform::clipboard::read());
            }
            Request::WriteFiles { paths, cut, reply } => {
                let result = naygo_platform::clipboard::write_files(&paths, cut)
                    .map_err(|error| format!("{error:?}"));
                let _ = reply.send(result);
            }
            Request::WriteText { text, reply } => {
                let result = naygo_platform::clipboard::write_text(&text)
                    .map_err(|error| format!("{error:?}"));
                let _ = reply.send(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread::ThreadId;

    fn test_io(mut process: impl FnMut(Request) + Send + 'static) -> ClipboardIo {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("test-clipboard-io".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    process(request);
                }
            })
            .expect("worker de prueba");
        ClipboardIo { sender }
    }

    #[test]
    fn write_files_runs_off_caller_thread_and_preserves_request_order() {
        let caller = std::thread::current().id();
        let worker_id = Arc::new(Mutex::new(None::<ThreadId>));
        let worker_id_copy = worker_id.clone();
        let operations = Arc::new(Mutex::new(Vec::new()));
        let operations_copy = operations.clone();
        let io = test_io(move |request| {
            *worker_id_copy.lock().unwrap() = Some(std::thread::current().id());
            match request {
                Request::WriteFiles { paths, cut, reply } => {
                    operations_copy
                        .lock()
                        .unwrap()
                        .push((paths.as_ref().to_vec(), cut));
                    let _ = reply.send(Ok(()));
                }
                _ => panic!("tipo de operación inesperado"),
            }
        });
        let handle = io.handle();

        let first = handle
            .write_files(vec![PathBuf::from("first.txt")], false)
            .expect("primera escritura");
        let second = handle
            .write_files(vec![PathBuf::from("second.txt")], true)
            .expect("segunda escritura");

        assert_eq!(first.receiver.recv().unwrap(), Ok(()));
        assert_eq!(second.receiver.recv().unwrap(), Ok(()));
        assert_ne!(*worker_id.lock().unwrap(), Some(caller));
        assert_eq!(
            *operations.lock().unwrap(),
            vec![
                (vec![PathBuf::from("first.txt")], false),
                (vec![PathBuf::from("second.txt")], true),
            ]
        );
    }

    #[test]
    fn queued_read_observes_prior_clipboard_write() {
        let operations = Arc::new(Mutex::new(Vec::<String>::new()));
        let operations_copy = operations.clone();
        let io = test_io(move |request| match request {
            Request::WriteText { text, reply } => {
                operations_copy
                    .lock()
                    .unwrap()
                    .push(format!("write:{text}"));
                let _ = reply.send(Ok(()));
            }
            Request::Read(reply) => {
                operations_copy.lock().unwrap().push("read".to_string());
                let _ = reply.send(ClipboardContent::Empty);
            }
            _ => panic!("tipo de operación inesperado"),
        });
        let handle = io.handle();
        let write = handle.write_text("path".into()).expect("escritura");
        let read = handle.read_async().expect("lectura");

        assert_eq!(write.receiver.recv().unwrap(), Ok(()));
        assert!(matches!(read.recv().unwrap(), ClipboardContent::Empty));
        assert_eq!(
            *operations.lock().unwrap(),
            vec!["write:path".to_string(), "read".to_string()]
        );
    }

    #[test]
    fn async_read_is_processed_off_caller_thread() {
        let caller = std::thread::current().id();
        let worker_id = Arc::new(Mutex::new(None::<ThreadId>));
        let worker_id_copy = worker_id.clone();
        let io = test_io(move |request| match request {
            Request::Read(reply) => {
                *worker_id_copy.lock().unwrap() = Some(std::thread::current().id());
                let _ = reply.send(ClipboardContent::Empty);
            }
            _ => panic!("tipo de operación inesperado"),
        });
        let read = io.handle().read_async().expect("lectura encolada");

        assert!(matches!(read.recv().unwrap(), ClipboardContent::Empty));
        assert_ne!(*worker_id.lock().unwrap(), Some(caller));
    }
}
