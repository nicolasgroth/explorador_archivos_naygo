// Naygo — lector persistente de animación, una respuesta por crédito del consumidor.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use naygo_core::preview_animation_wire::{self as wire, Event};
use std::{
    io::{Read, Write},
    sync::mpsc::{self, Receiver, SyncSender},
};

/// Runs on an already reserved preview worker. Neither UI nor its Drop joins a thread.
/// Credits and events have capacity one. Pause issues no credits; cancellation bypasses
/// both queues. Only an outstanding request has a deadline, not valid frame delays/pauses.
pub fn run(
    executable: &Path,
    request: Vec<u8>,
    token: &CancellationToken,
    credits: Receiver<()>,
    events: SyncSender<Result<Event, Error>>,
) -> Result<usize, Error> {
    if token.is_cancelled() {
        return Err(Error::Cancelled);
    }
    if request.is_empty() || request.len() > preview_wire::REQUEST_MAX {
        return Err(Error::Failed);
    }
    let mut command = Command::new(executable);
    command
        .arg(ANIMATION_ARGUMENT)
        .env("RAYON_NUM_THREADS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00004000);
    }
    let mut process = Process(command.spawn().map_err(|_| Error::Failed)?);
    let job = Job::attach_mode(&process.0, true).map_err(|_| Error::Failed)?;
    let mut input = process.0.stdin.take().ok_or(Error::Failed)?;
    let mut output = process.0.stdout.take().ok_or(Error::Failed)?;
    let (pull_tx, pull_rx) = mpsc::sync_channel(1);
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    let pipe = std::thread::Builder::new()
        .name("naygo-animation-pipe".into())
        .spawn(move || {
            let result = (|| -> io::Result<()> {
                preview_wire::write_request(&mut input, &request)?;
                loop {
                    if pull_rx.recv().is_err() {
                        return Ok(());
                    }
                    input.write_all(&[1])?;
                    input.flush()?;
                    let event = wire::read(&mut output)?;
                    let done = !matches!(event, Event::Frame { .. });
                    if done {
                        let mut extra = [0];
                        if output.read(&mut extra)? != 0 {
                            return Err(io::Error::other("extra animation output"));
                        }
                    }
                    if reply_tx.send(Ok(event)).is_err() || done {
                        return Ok(());
                    }
                }
            })();
            if result.is_err() {
                let _ = reply_tx.send(Err(Error::Failed));
            }
        })
        .map_err(|_| Error::Failed)?;
    let mut outstanding = None;
    let mut previous_at = None;
    let result = 'supervisor: loop {
        if token.is_cancelled() {
            break Err(Error::Cancelled);
        }
        if let Some(started) = outstanding {
            if Instant::now().duration_since(started) >= WALL_LIMIT {
                break Err(Error::Timeout);
            }
            match reply_rx.recv_timeout(Duration::from_millis(25)) {
                Ok(Ok(event)) => {
                    let done = !matches!(event, Event::Frame { .. });
                    if done {
                        loop {
                            match process.0.try_wait() {
                                Ok(Some(status)) if status.success() => break,
                                Ok(None)
                                    if !token.is_cancelled() && started.elapsed() < WALL_LIMIT =>
                                {
                                    std::thread::sleep(Duration::from_millis(5));
                                }
                                _ => break 'supervisor Err(Error::Failed),
                            }
                        }
                    }
                    let at = match &event {
                        Event::Frame { at_us, .. } | Event::End { at_us } => Some(*at_us),
                        _ => None,
                    };
                    if at.zip(previous_at).is_some_and(|(a, b)| a < b) {
                        break Err(Error::Failed);
                    }
                    previous_at = at;
                    // A full queue indicates a broken credit contract, never block Cancel behind it.
                    if events.try_send(Ok(event)).is_err() {
                        break Err(Error::Failed);
                    }
                    outstanding = None;
                    if done {
                        break Ok(job.peak());
                    }
                }
                Ok(Err(error)) => break Err(error),
                Err(mpsc::RecvTimeoutError::Disconnected) => break Err(Error::Failed),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        } else {
            match credits.recv_timeout(Duration::from_millis(25)) {
                Ok(()) => {
                    if pull_tx.try_send(()).is_err() {
                        break Err(Error::Failed);
                    }
                    outstanding = Some(Instant::now());
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break Err(Error::Cancelled),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !matches!(process.0.try_wait(), Ok(None)) {
                        break Err(Error::Failed);
                    }
                }
            }
        }
    };
    drop(process);
    #[cfg(windows)]
    drop(job);
    drop(pull_tx);
    drop(reply_rx);
    let _ = pipe.join();
    result
}
