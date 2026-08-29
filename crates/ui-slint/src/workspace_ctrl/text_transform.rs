// Naygo — flujo UI/worker para transformar finales de línea y codificación.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use crate::TextTransformVm;
use naygo_core::text_transform::{
    FinalNewline, LineEnding, TargetEncoding, TextDiagnosis, TextEncoding, TransformOptions,
};
use slint::SharedString;

enum TextJobMsg {
    Progress(usize),
    Done(usize),
    Cancelled,
}

pub struct TextTransformState {
    pub paths: Vec<PathBuf>,
    pub options: TransformOptions,
    pub diagnosis: Option<TextDiagnosis>,
    pub diagnosis_error: Option<String>,
    diagnosis_rx: Option<std::sync::mpsc::Receiver<Result<TextDiagnosis, String>>>,
    job_rx: Option<std::sync::mpsc::Receiver<TextJobMsg>>,
    pub token: naygo_core::CancellationToken,
    pub completed: usize,
    pub failed: usize,
    pub finished: bool,
}

impl WorkspaceCtrl {
    pub fn text_transform_open(&mut self) -> bool {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return false;
        }
        if let Some(old) = self.text_transform.take() {
            old.token.cancel();
        }
        let token = naygo_core::CancellationToken::new();
        let worker_token = token.clone();
        let first = paths[0].clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            if worker_token.is_cancelled() {
                return;
            }
            let result = std::fs::metadata(&first)
                .map_err(|error| error.to_string())
                .and_then(|metadata| {
                    if !metadata.is_file()
                        || metadata.len() > naygo_core::text_transform::MAX_TEXT_TRANSFORM_BYTES
                    {
                        return Err("archivo no válido o demasiado grande".to_string());
                    }
                    std::fs::read(&first).map_err(|error| error.to_string())
                })
                .and_then(|bytes| {
                    naygo_core::text_transform::diagnose_bytes(&bytes)
                        .map_err(|error| format!("{error:?}"))
                });
            let _ = tx.send(result);
        });
        self.text_transform = Some(TextTransformState {
            paths,
            options: TransformOptions {
                line_ending: LineEnding::Lf,
                encoding: TargetEncoding::Preserve,
                final_newline: FinalNewline::Preserve,
                trim_trailing_whitespace: false,
            },
            diagnosis: None,
            diagnosis_error: None,
            diagnosis_rx: Some(rx),
            job_rx: None,
            token,
            completed: 0,
            failed: 0,
            finished: false,
        });
        true
    }

    pub fn text_transform_set_line(&mut self, value: i32) {
        if let Some(state) = self.text_transform.as_mut() {
            state.options.line_ending = match value {
                0 => LineEnding::Crlf,
                2 => LineEnding::Cr,
                _ => LineEnding::Lf,
            };
        }
    }

    pub fn text_transform_set_encoding(&mut self, value: i32) {
        if let Some(state) = self.text_transform.as_mut() {
            state.options.encoding = match value {
                1 => TargetEncoding::Utf8,
                2 => TargetEncoding::Utf8Bom,
                3 => TargetEncoding::Utf16Le,
                4 => TargetEncoding::Utf16Be,
                _ => TargetEncoding::Preserve,
            };
        }
    }

    pub fn text_transform_set_final(&mut self, value: i32) {
        if let Some(state) = self.text_transform.as_mut() {
            state.options.final_newline = match value {
                1 => FinalNewline::Ensure,
                2 => FinalNewline::Remove,
                _ => FinalNewline::Preserve,
            };
        }
    }

    pub fn text_transform_set_trim(&mut self, value: bool) {
        if let Some(state) = self.text_transform.as_mut() {
            state.options.trim_trailing_whitespace = value;
        }
    }

    pub fn text_transform_apply(&mut self) -> bool {
        let Some(state) = self.text_transform.as_mut() else {
            return false;
        };
        if state.job_rx.is_some() || state.finished {
            return false;
        }
        let paths = state.paths.clone();
        let options = state.options;
        let token = state.token.clone();
        let worker_token = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut failed = 0;
            for (index, path) in paths.iter().enumerate() {
                if worker_token.is_cancelled() {
                    let _ = tx.send(TextJobMsg::Cancelled);
                    return;
                }
                match naygo_core::text_transform::transform_file(path, options, &worker_token) {
                    Ok(outcome) => {
                        // El respaldo cumplió su función transaccional. El historial general no
                        // puede restaurar bytes sobrescritos aún, por lo que no dejamos archivos
                        // ocultos huérfanos indefinidamente.
                        let _ = std::fs::remove_file(outcome.backup_path);
                    }
                    Err(_) => failed += 1,
                }
                let _ = tx.send(TextJobMsg::Progress(index + 1));
            }
            let _ = tx.send(TextJobMsg::Done(failed));
        });
        state.job_rx = Some(rx);
        true
    }

    pub fn text_transform_close(&mut self) {
        if let Some(state) = self.text_transform.take() {
            state.token.cancel();
        }
    }

    pub fn pump_text_transform(&mut self) -> bool {
        let Some(state) = self.text_transform.as_mut() else {
            return true;
        };
        if let Some(rx) = state.diagnosis_rx.as_ref() {
            match rx.try_recv() {
                Ok(Ok(diagnosis)) => {
                    state.diagnosis = Some(diagnosis);
                    state.diagnosis_rx = None;
                }
                Ok(Err(error)) => {
                    state.diagnosis_error = Some(error);
                    state.diagnosis_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    state.diagnosis_error = Some(self.config.t("text_transform.invalid"));
                    state.diagnosis_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            }
        }
        let mut job_done = state.job_rx.is_none();
        if let Some(rx) = state.job_rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(TextJobMsg::Progress(done)) => state.completed = done,
                    Ok(TextJobMsg::Done(failed)) => {
                        state.failed = failed;
                        state.finished = true;
                        job_done = true;
                    }
                    Ok(TextJobMsg::Cancelled) => {
                        state.finished = true;
                        job_done = true;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        state.finished = true;
                        job_done = true;
                        break;
                    }
                }
            }
            if job_done {
                state.job_rx = None;
            }
        }
        state.diagnosis_rx.is_none() && job_done
    }

    pub fn text_transform_vm(&self) -> TextTransformVm {
        let Some(state) = self.text_transform.as_ref() else {
            return TextTransformVm::default();
        };
        let source = state
            .diagnosis
            .as_ref()
            .map(|diagnosis| {
                format!(
                    "{} · {}",
                    encoding_label(diagnosis.encoding),
                    self.config.t(ending_key(diagnosis.line_ending))
                )
            })
            .or_else(|| state.diagnosis_error.clone())
            .unwrap_or_else(|| self.config.t("text_transform.analyzing"));
        let target_line = match state.options.line_ending {
            LineEnding::Crlf => 0,
            LineEnding::Cr => 2,
            _ => 1,
        };
        let target_encoding = match state.options.encoding {
            TargetEncoding::Preserve => 0,
            TargetEncoding::Utf8 => 1,
            TargetEncoding::Utf8Bom => 2,
            TargetEncoding::Utf16Le => 3,
            TargetEncoding::Utf16Be => 4,
        };
        let final_newline = match state.options.final_newline {
            FinalNewline::Preserve => 0,
            FinalNewline::Ensure => 1,
            FinalNewline::Remove => 2,
        };
        let running = state.job_rx.is_some();
        let status = if state.finished {
            format!(
                "{} / {} · {}: {}",
                state.completed,
                state.paths.len(),
                self.config.t("text_transform.failed"),
                state.failed
            )
        } else if running {
            format!("{} / {}", state.completed, state.paths.len())
        } else {
            String::new()
        };
        TextTransformVm {
            active: true,
            count: state.paths.len() as i32,
            source: SharedString::from(source),
            target_line,
            target_encoding,
            final_newline,
            trim: state.options.trim_trailing_whitespace,
            running,
            finished: state.finished,
            status: SharedString::from(status),
            can_apply: state.diagnosis.is_some() && !running && !state.finished,
        }
    }
}

fn encoding_label(encoding: TextEncoding) -> &'static str {
    match encoding {
        TextEncoding::Utf8 => "UTF-8",
        TextEncoding::Utf8Bom => "UTF-8 BOM",
        TextEncoding::Utf16Le => "UTF-16 LE",
        TextEncoding::Utf16Be => "UTF-16 BE",
    }
}

fn ending_key(ending: LineEnding) -> &'static str {
    match ending {
        LineEnding::Crlf => "text_transform.windows",
        LineEnding::Lf => "text_transform.unix",
        LineEnding::Cr => "text_transform.mac_classic",
        LineEnding::Mixed => "text_transform.mixed",
        LineEnding::None => "text_transform.no_endings",
    }
}
