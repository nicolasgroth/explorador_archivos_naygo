// Naygo — adaptador del preview a lectores aislados y dispatcher anterior a la UI.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use naygo_core::{
    archive_tree::ArchiveLabels,
    preview_wire::{self, Response},
};
use naygo_platform::preview_process;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    #[serde(default)]
    economical: bool,
    #[serde(default = "default_text_max_bytes")]
    text_max_bytes: usize,
    #[serde(default)]
    lang: Option<CodeLang>,
    path: PathBuf,
    kind: Kind,
    messages: PreviewMessages,
    labels: [String; 5],
}
#[derive(Serialize, Deserialize)]
enum Kind {
    Image,
    Svg,
    Pdf,
    Archive,
    Text,
    Mesh,
    Scene,
}

/// Limits and rendering policy captured when a preview worker is dispatched.
#[derive(Clone, Copy)]
pub(super) struct BuildOptions {
    pub quality: naygo_core::preview_policy::Quality,
    pub text_max_bytes: usize,
}

fn default_text_max_bytes() -> usize {
    naygo_core::preview::DEFAULT_TEXT_MAX_BYTES as usize
}

/// Routing is in-memory and preserves classification/disabled rules. The primary worker
/// owns the subprocess until confirmed exit, so the existing two-slot budget includes
/// canceling readers and their pipe threads. ALL preview filesystem/parser work is isolated;
/// camera gestures retain geometry and use the bounded, cooperative in-memory renderer.
pub(super) fn build(
    path: &Path,
    rules: &[PreviewRule],
    auto_highlight: bool,
    options: BuildOptions,
    labels: &ArchiveLabels,
    messages: &PreviewMessages,
    token: &CancellationToken,
) -> Payload {
    if token.is_cancelled() {
        return Payload::Message(messages.cancelled.clone());
    }
    let kind = if is_archive(path) {
        Some(Kind::Archive)
    } else {
        match preview::classify_rules(path, rules) {
            PreviewKind::Image => Some(Kind::Image),
            PreviewKind::Svg => Some(Kind::Svg),
            PreviewKind::Pdf => Some(Kind::Pdf),
            PreviewKind::Text => Some(Kind::Text),
            PreviewKind::Mesh => Some(Kind::Mesh),
            _ => None,
        }
    };
    let Some(kind) = kind else {
        return Payload::Message(messages.not_previewable.clone());
    };
    if token.is_cancelled() {
        return Payload::Message(messages.cancelled.clone());
    }
    let request = Request {
        economical: options.quality.economical,
        text_max_bytes: naygo_core::preview::normalize_text_max_bytes(
            options
                .text_max_bytes
                .min(naygo_core::preview::TEXT_MAX_BYTES) as u32,
        ) as usize,
        lang: preview::code_lang_for(path, rules, auto_highlight),
        path: path.to_owned(),
        kind,
        messages: messages.clone(),
        labels: [
            labels.files.clone(),
            labels.folders.clone(),
            labels.uncompressed.clone(),
            labels.more_entries.clone(),
            labels.and_more.clone(),
        ],
    };
    match run(&request, token) {
        Some(Ok(output)) => from_wire(output.response)
            .unwrap_or_else(|_| Payload::Message(messages.reader_stopped.clone())),
        Some(Err(preview_process::Error::Cancelled)) => {
            Payload::Message(messages.cancelled.clone())
        }
        // Terminal, explicitly retryable; no uncontained fallback after reader failure.
        _ => Payload::Message(messages.reader_stopped.clone()),
    }
}

fn run(
    request: &Request,
    token: &CancellationToken,
) -> Option<Result<preview_process::Output, preview_process::Error>> {
    std::env::current_exe().ok().and_then(|exe| {
        serde_json::to_vec(request)
            .ok()
            .map(|bytes| preview_process::run(&exe, bytes, token))
    })
}

pub(super) fn load_scene(
    path: &Path,
    token: &CancellationToken,
) -> Result<naygo_core::mesh_preview::MeshScene, naygo_core::mesh_preview::MeshError> {
    use naygo_core::mesh_preview::MeshError;
    let request = Request {
        economical: false,
        text_max_bytes: default_text_max_bytes(),
        lang: None,
        path: path.to_owned(),
        kind: Kind::Scene,
        messages: PreviewMessages::default(),
        labels: Default::default(),
    };
    match run(&request, token) {
        Some(Ok(output)) => match output.response {
            Response::Scene(scene) => Ok(scene),
            Response::Message(text) if text == request.messages.image_big => {
                Err(MeshError::TooLarge)
            }
            Response::Message(text) if text == request.messages.decode => Err(MeshError::Invalid),
            _ => Err(MeshError::ReaderStopped),
        },
        Some(Err(preview_process::Error::Cancelled)) => Err(MeshError::Cancelled),
        _ => Err(MeshError::ReaderStopped),
    }
}

pub(super) fn from_wire(response: Response) -> Result<Payload, preview_process::Error> {
    Ok(match response {
        Response::Image {
            width,
            height,
            rgba,
        } => Payload::Image {
            width,
            height,
            rgba,
        },
        Response::Text {
            text,
            truncated,
            bytes_read,
        } => Payload::Text {
            text,
            truncated,
            bytes_read: bytes_read.map(|bytes| bytes as usize),
            highlighted: None,
        },
        Response::Message(text) => Payload::Message(text),
        Response::StyledText {
            text,
            truncated,
            bytes_read,
            lines,
        } => Payload::Text {
            text,
            truncated,
            bytes_read: bytes_read.map(|bytes| bytes as usize),
            highlighted: Some(lines),
        },
        // Scene responses are accepted only by the geometry loader, not the UI payload path.
        Response::Scene(_) => return Err(preview_process::Error::Failed),
    })
}

fn serve() -> std::io::Result<()> {
    let bytes = preview_wire::read_request(&mut std::io::stdin().lock())?;
    if !preview_process::reader_is_contained() {
        return Err(std::io::Error::other("preview reader requires containment"));
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    let token = CancellationToken::new();
    let mut output = std::io::BufWriter::with_capacity(64 * 1024, std::io::stdout().lock());
    if matches!(request.kind, Kind::Scene) {
        let response = match naygo_core::mesh_preview::load_scene(&request.path, &token) {
            Ok(scene) if scene.wire_len().is_ok() => Response::Scene(scene),
            Ok(_) => Response::Message(request.messages.image_big.clone()),
            Err(error) => to_wire(mesh_error_payload(error, &request.messages)),
        };
        return preview_wire::write_response(&mut output, &response);
    }
    let payload = match request.kind {
        Kind::Image => read_image_with_quality(
            &request.path,
            &token,
            &request.messages,
            Quality {
                economical: request.economical,
            },
        ),
        Kind::Svg => read_svg_with_quality(
            &request.path,
            &token,
            &request.messages,
            Quality {
                economical: request.economical,
            },
        ),
        Kind::Pdf => read_pdf(&request.path, &request.messages),
        Kind::Text => read_text_with_limit(
            &request.path,
            request.lang,
            &request.messages,
            request.text_max_bytes,
        ),
        Kind::Mesh => read_mesh_with_quality(
            &request.path,
            &token,
            &request.messages,
            Quality {
                economical: request.economical,
            },
        ),
        Kind::Scene => unreachable!(),
        Kind::Archive => {
            let [files, folders, uncompressed, more_entries, and_more] = request.labels;
            let labels = ArchiveLabels {
                files,
                folders,
                uncompressed,
                more_entries,
                and_more,
            };
            read_archive_listing(&request.path, &labels, &request.messages)
        }
    };
    preview_wire::write_response(&mut output, &to_wire(payload))
}

fn to_wire(payload: Payload) -> Response {
    match payload {
        Payload::Image {
            width,
            height,
            rgba,
        } => Response::Image {
            width,
            height,
            rgba,
        },
        Payload::Text {
            text,
            truncated,
            bytes_read,
            highlighted: Some(lines),
        } => Response::StyledText {
            text,
            truncated,
            bytes_read: bytes_read.map(|bytes| bytes.min(u32::MAX as usize) as u32),
            lines,
        },
        Payload::Text {
            text,
            truncated,
            bytes_read,
            highlighted: None,
        } => Response::Text {
            text,
            truncated,
            bytes_read: bytes_read.map(|bytes| bytes.min(u32::MAX as usize) as u32),
        },
        Payload::Message(text) => Response::Message(text),
    }
}

pub(super) fn stream(
    path: PathBuf,
    messages: PreviewMessages,
    quality: Quality,
    token: CancellationToken,
    credits: std::sync::mpsc::Receiver<()>,
    events: std::sync::mpsc::SyncSender<
        Result<naygo_core::preview_animation_wire::Event, preview_process::Error>,
    >,
) {
    let result = (|| {
        let request = Request {
            economical: quality.economical,
            text_max_bytes: default_text_max_bytes(),
            lang: None,
            path,
            kind: Kind::Image,
            messages,
            labels: Default::default(),
        };
        let bytes = serde_json::to_vec(&request).map_err(|_| preview_process::Error::Failed)?;
        let exe = std::env::current_exe().map_err(|_| preview_process::Error::Failed)?;
        preview_process::run_animation(&exe, bytes, &token, credits, events.clone())
    })();
    if let Err(error) = result {
        let _ = events.try_send(Err(error));
    }
}

fn serve_animation() -> std::io::Result<()> {
    use naygo_core::preview_animation_wire::{self as wire, Event};
    use std::io::Read;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let bytes = preview_wire::read_request(&mut input)?;
    if !preview_process::animation_reader_is_contained() {
        return Err(std::io::Error::other(
            "animation reader requires containment",
        ));
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
    if !matches!(request.kind, Kind::Image) {
        return Err(std::io::Error::other("invalid animation request"));
    }
    fn credit(input: &mut impl Read) -> std::io::Result<()> {
        let mut command = [0];
        input.read_exact(&mut command)?;
        if command != [1] {
            return Err(std::io::Error::other("invalid animation credit"));
        }
        Ok(())
    }
    credit(&mut input)?;
    let result = (|| {
        let bytes = read_bounded(&request.path, preview::IMAGE_MAX_BYTES)?;
        let quality = Quality {
            economical: request.economical,
        };
        let Some(mut reader) = super::animation_reader::Reader::with_quality(&bytes, quality)?
        else {
            return wire::write(
                &mut output,
                &Event::Static(to_wire(read_image_with_quality(
                    &request.path,
                    &CancellationToken::new(),
                    &request.messages,
                    quality,
                ))),
            );
        };
        loop {
            let event = reader.next()?;
            let done = matches!(event, Event::End { .. });
            wire::write(&mut output, &event)?;
            if done {
                return Ok(());
            }
            credit(&mut input)?;
        }
    })();
    if result.is_err() {
        wire::write(
            &mut output,
            &Event::Static(Response::Message(request.messages.reader_stopped)),
        )?;
    }
    Ok(())
}

pub fn dispatch() -> bool {
    let argument = std::env::args_os().nth(1);
    let serve: fn() -> std::io::Result<()> = if argument.as_deref()
        == Some(std::ffi::OsStr::new(preview_process::ARGUMENT))
    {
        serve
    } else if argument.as_deref() == Some(std::ffi::OsStr::new(preview_process::ANIMATION_ARGUMENT))
    {
        serve_animation
    } else {
        return false;
    };
    // No dialogs, logs or settings in this mode, including panic. The supervisor validates
    // exit status and EOF and reports a discrete translated error in the parent preview.
    std::panic::set_hook(Box::new(|_| {}));
    preview_process::silent_reader_errors();
    if !matches!(std::panic::catch_unwind(serve), Ok(Ok(()))) {
        std::process::exit(2);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_image_rule_does_not_launch_reader_or_touch_path() {
        let messages = PreviewMessages::default();
        let mut rules = preview::default_preview_rules();
        rules
            .iter_mut()
            .find(|rule| rule.ext == "webp")
            .unwrap()
            .enabled = false;
        let state = PreviewState::new();
        let payload = build(
            Path::new("missing.WEBP"),
            &rules,
            true,
            BuildOptions {
                quality: Quality::default(),
                text_max_bytes: default_text_max_bytes(),
            },
            &state.archive_labels,
            &messages,
            &CancellationToken::new(),
        );
        assert!(matches!(payload, Payload::Message(text) if text == messages.not_previewable));
    }
    #[test]
    fn cancellation_before_isolation_avoids_starting_process() {
        let messages = PreviewMessages::default();
        let token = CancellationToken::new();
        token.cancel();
        let state = PreviewState::new();
        let payload = build(
            Path::new("missing.webp"),
            &state.rules,
            true,
            BuildOptions {
                quality: Quality::default(),
                text_max_bytes: default_text_max_bytes(),
            },
            &state.archive_labels,
            &messages,
            &token,
        );
        assert!(matches!(payload, Payload::Message(text) if text == messages.cancelled));
    }
}
