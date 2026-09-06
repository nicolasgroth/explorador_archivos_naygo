// Naygo — WorkspaceCtrl: worker async de metadata por tipo (dimensiones, versión).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Lee la metadata extra de un archivo (imagen: dimensiones; exe/dll: versión) EN UN HILO
//! aparte, para que el hilo de UI nunca toque el disco. Un solo job a la vez: al enfocar otro
//! archivo se cancela el anterior y su resultado se descarta si ya no corresponde. El resultado
//! se drena en el tick (`pump_meta`), igual que `SizeJob`/`pump_sizes`.

use super::*;

/// Resultado del worker de metadata. La presencia del ADS se mantiene separada de sus campos:
/// Windows puede crear un Zone.Identifier sin ZoneId/URLs legibles y aun así debe poder quitarse.
pub(crate) struct MetaResult {
    basic: Result<std::fs::Metadata, String>,
    fields: Vec<naygo_core::metadata::MetadataField>,
    has_zone_identifier: bool,
}

/// Lectura de metadata en curso/terminada para el archivo actualmente enfocado.
pub struct MetaJob {
    pub basic: Option<Result<std::fs::Metadata, String>>,
    /// Archivo cuya metadata se pidió (clave para no relanzar el mismo job en cada tick).
    pub path: std::path::PathBuf,
    /// Canal por el que el worker envía los campos leídos (una sola vez).
    pub rx: std::sync::mpsc::Receiver<MetaResult>,
    /// Token para descartar el resultado si se enfoca otro archivo antes de que termine.
    pub token: naygo_core::CancellationToken,
    /// Campos ya recibidos: (clave i18n de la etiqueta, valor formateado). La traducción de la
    /// etiqueta ocurre al construir el VM (main.rs, con `config.t`), no aquí.
    pub fields: Vec<(String, String)>,
    /// El stream `Zone.Identifier` existe aunque no haya entregado valores que mostrar.
    pub has_zone_identifier: bool,
    /// El worker ya entregó su resultado.
    pub done: bool,
}

impl WorkspaceCtrl {
    /// Copia al portapapeles los campos de procedencia ya resueltos. No relee ADS ni toca disco:
    /// si todavía no existe un resultado, el botón simplemente no hace nada.
    pub fn copy_provenance(&self) -> bool {
        let Some(job) = self.meta_job.as_ref() else {
            return false;
        };
        let text = job
            .fields
            .iter()
            .filter(|(key, _)| {
                matches!(
                    key.as_str(),
                    "meta.zone" | "meta.host_url" | "meta.referrer_url"
                )
            })
            .map(|(key, value)| format!("{}: {value}", self.config.t(key)))
            .collect::<Vec<_>>()
            .join("\r\n");
        if text.is_empty() {
            return false;
        }
        naygo_platform::clipboard::write_text(&text).is_ok()
    }

    /// Inicia la eliminación explícita de la marca de procedencia. El caller ya mostró la
    /// confirmación; esta función solo encola la escritura ADS en un worker y no bloquea la UI.
    pub fn unblock_provenance(&mut self) -> bool {
        if self.zone_unblock_rx.is_some() {
            return false;
        }
        let Some(path) = self.metadata_target() else {
            return false;
        };
        let worker_path = path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = naygo_platform::zone_identifier::remove(&worker_path)
                .map_err(|error| error.to_string());
            let _ = tx.send((worker_path, result));
        });
        self.zone_unblock_rx = Some(rx);
        true
    }

    /// Drena el desbloqueo. Tras éxito (incluso si ya no había ADS) vuelve a leer metadata en
    /// worker para que el Inspector desaparezca la procedencia sin hacer I/O en el hilo UI.
    pub fn pump_zone_unblock(&mut self) -> bool {
        let Some(rx) = self.zone_unblock_rx.as_ref() else {
            return true;
        };
        let Ok((path, result)) = rx.try_recv() else {
            return false;
        };
        self.zone_unblock_rx = None;
        match result {
            Ok(_) => {
                self.clear_metadata();
                self.request_metadata(path);
            }
            Err(error) => self.pending_shell_error = Some(error),
        }
        true
    }

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
            if worker_token.is_cancelled() {
                return;
            }
            let basic = std::fs::metadata(&worker_path).map_err(|e| e.to_string());
            if worker_token.is_cancelled() {
                return;
            }
            let mut fields = naygo_core::metadata::metadata_for(&worker_path);
            let mut has_zone_identifier = false;
            match naygo_platform::zone_identifier::read(&worker_path) {
                Ok(Some(zone)) => {
                    has_zone_identifier = true;
                    if let Some(id) = zone.zone_id {
                        fields.push(naygo_core::metadata::MetadataField {
                            label_key: "meta.zone",
                            value: id.to_string(),
                        });
                    }
                    if let Some(url) = zone.host_url {
                        fields.push(naygo_core::metadata::MetadataField {
                            label_key: "meta.host_url",
                            value: url,
                        });
                    }
                    if let Some(url) = zone.referrer_url {
                        fields.push(naygo_core::metadata::MetadataField {
                            label_key: "meta.referrer_url",
                            value: url,
                        });
                    }
                }
                Ok(None) => {}
                Err(error) => fields.push(naygo_core::metadata::MetadataField {
                    label_key: "meta.zone_status",
                    value: error.to_string(),
                }),
            }
            // Si ya se enfocó otro archivo, el receptor descarta el mensaje; no importa si el
            // send falla (canal cerrado porque el job fue reemplazado).
            if !worker_token.is_cancelled() {
                let _ = tx.send(MetaResult {
                    basic,
                    fields,
                    has_zone_identifier,
                });
            }
        });
        self.meta_job = Some(MetaJob {
            basic: None,
            path,
            rx,
            token,
            fields: Vec::new(),
            has_zone_identifier: false,
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
        if let Ok(result) = job.rx.try_recv() {
            job.basic = Some(result.basic);
            job.fields = result
                .fields
                .into_iter()
                .map(|f| (f.label_key.to_string(), f.value))
                .collect();
            job.has_zone_identifier = result.has_zone_identifier;
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

    /// `true` si el archivo tiene un stream Zone.Identifier, incluso cuando Windows no expone
    /// ZoneId, HostUrl ni ReferrerUrl. Así la acción de desbloquear nunca queda inaccesible.
    pub fn has_provenance(&self) -> bool {
        matches!(self.meta_job.as_ref(), Some(job) if job.done && job.has_zone_identifier)
    }

    /// Archivo cuya metadata debe mostrarse, o `None` si no aplica (carpeta / nada enfocado).
    /// Sigue la MISMA regla que el panel de Vista previa (`drive_preview`): el ítem enfocado del
    /// ÚLTIMO panel Files activo (no el panel activo a secas), con fallback al Files activo. Así,
    /// con un panel Preview abierto, la metadata corresponde SIEMPRE al archivo que se está
    /// previsualizando, y hacer clic dentro del propio Preview/Inspector no la vacía. Solo
    /// archivos: para carpetas no hay metadata por tipo.
    pub fn metadata_target(&self) -> Option<std::path::PathBuf> {
        if self.search_context {
            return self
                .search_selected_entry()
                .filter(|e| e.kind != EntryKind::Directory)
                .map(|e| e.path.clone());
        }
        self.basket_selected_path().or_else(|| {
            self.last_active_files
                .and_then(|id| self.ws.pane(id))
                .and_then(|p| p.files.as_ref())
                .or_else(|| self.ws.active_files())
                .and_then(|f| f.focused_view_entry())
                .filter(|e| e.kind != naygo_core::fs_model::EntryKind::Directory)
                .map(|e| e.path.clone())
        })
    }
}
