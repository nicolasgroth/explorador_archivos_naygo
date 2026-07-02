// Naygo — WorkspaceCtrl: worker async de metadata por tipo (dimensiones, versión).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Lee la metadata extra de un archivo (imagen: dimensiones; exe/dll: versión) EN UN HILO
//! aparte, para que el hilo de UI nunca toque el disco. Un solo job a la vez: al enfocar otro
//! archivo se cancela el anterior y su resultado se descarta si ya no corresponde. El resultado
//! se drena en el tick (`pump_meta`), igual que `SizeJob`/`pump_sizes`.

use super::*;

/// Lectura de metadata en curso/terminada para el archivo actualmente enfocado.
pub struct MetaJob {
    /// Archivo cuya metadata se pidió (clave para no relanzar el mismo job en cada tick).
    pub path: std::path::PathBuf,
    /// Canal por el que el worker envía los campos leídos (una sola vez).
    pub rx: std::sync::mpsc::Receiver<Vec<naygo_core::metadata::MetadataField>>,
    /// Token para descartar el resultado si se enfoca otro archivo antes de que termine.
    pub token: naygo_core::CancellationToken,
    /// Campos ya recibidos: (clave i18n de la etiqueta, valor formateado). La traducción de la
    /// etiqueta ocurre al construir el VM (main.rs, con `config.t`), no aquí.
    pub fields: Vec<(String, String)>,
    /// El worker ya entregó su resultado.
    pub done: bool,
}

impl WorkspaceCtrl {
    /// Pide la metadata del archivo `path`. Si ya hay un job VIVO para ese mismo `path`, no
    /// relanza (evita re-lanzar en cada tick mientras se muestra el mismo archivo). Si es otro
    /// path, cancela el job anterior y lanza uno nuevo en un hilo worker.
    ///
    /// La UI se despierta sola: el tick corre por un timer periódico (30 ms) que sigue vivo
    /// mientras `pump_meta` devuelva `false`; cuando el worker termina, el siguiente tick drena
    /// el canal. No hace falta un `Waker` (el timer ya está corriendo al pedir la metadata).
    pub fn request_metadata(&mut self, path: std::path::PathBuf) {
        // ¿Ya estamos leyendo (o mostrando) la metadata de este mismo archivo? No relanzar.
        if let Some(job) = self.meta_job.as_ref() {
            if job.path == path {
                return;
            }
        }
        // Enfocamos otro archivo: cancelar el job anterior (su resultado se descartará).
        if let Some(job) = self.meta_job.take() {
            job.token.cancel();
        }
        let token = naygo_core::CancellationToken::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_path = path.clone();
        let worker_token = token.clone();
        std::thread::spawn(move || {
            let fields = naygo_core::metadata::metadata_for(&worker_path);
            // Si ya se enfocó otro archivo, el receptor descarta el mensaje; no importa si el
            // send falla (canal cerrado porque el job fue reemplazado).
            if !worker_token.is_cancelled() {
                let _ = tx.send(fields);
            }
        });
        self.meta_job = Some(MetaJob {
            path,
            rx,
            token,
            fields: Vec::new(),
            done: false,
        });
    }

    /// Cancela y limpia el job de metadata en vuelo (al enfocar una carpeta o al perder el foco).
    /// Deja `meta_fields()`/`meta_loading()` vacíos.
    pub fn clear_metadata(&mut self) {
        if let Some(job) = self.meta_job.take() {
            job.token.cancel();
        }
    }

    /// Drena el worker de metadata en vuelo (si lo hay). Devuelve `true` si NO queda lectura
    /// pendiente (para que el timer pueda dormirse). El job terminado se conserva para que el VM
    /// siga mostrando los campos hasta que se enfoque otro archivo.
    pub fn pump_meta(&mut self) -> bool {
        let Some(job) = self.meta_job.as_mut() else {
            return true;
        };
        if job.done {
            return true;
        }
        // El worker envía una sola vez; `try_recv` no bloquea el hilo de UI.
        if let Ok(fields) = job.rx.try_recv() {
            job.fields = fields
                .into_iter()
                .map(|f| (f.label_key.to_string(), f.value))
                .collect();
            job.done = true;
        }
        job.done
    }

    /// Campos de metadata del archivo actual: (clave i18n de la etiqueta, valor). Vacío si no hay
    /// job o si el worker aún no respondió.
    pub fn meta_fields(&self) -> &[(String, String)] {
        match self.meta_job.as_ref() {
            Some(job) => &job.fields,
            None => &[],
        }
    }

    /// `true` si hay una lectura de metadata en vuelo (el VM muestra "Leyendo…").
    pub fn meta_loading(&self) -> bool {
        matches!(self.meta_job.as_ref(), Some(job) if !job.done)
    }
}
