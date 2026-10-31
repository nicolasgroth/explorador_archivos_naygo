// Naygo — reloj monotónico y una sola respuesta pendiente por generación del preview.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use naygo_core::preview_animation_wire::Event;
use naygo_platform::preview_process::Error;
use std::sync::mpsc::{self, Receiver, SyncSender};

pub(super) struct Playback {
    quality: Quality,
    credits: SyncSender<()>,
    events: Receiver<Result<Event, Error>>,
    token: CancellationToken,
    pending: Option<Event>,
    origin: Option<Instant>,
    paused: Option<Instant>,
    in_flight: bool,
    pub(super) animated: bool,
    pub(super) ended: bool,
    last_presented: Option<Instant>,
}
impl Playback {
    pub(super) fn start(
        workers: &workers::Workers,
        path: PathBuf,
        messages: PreviewMessages,
        quality: Quality,
    ) -> std::io::Result<Option<Self>> {
        let (credits, credit_rx) = mpsc::sync_channel(1);
        let (events_tx, events) = mpsc::sync_channel(1);
        let token = CancellationToken::new();
        let worker_token = token.clone();
        if workers
            .spawn(move || {
                isolated::stream(path, messages, quality, worker_token, credit_rx, events_tx)
            })?
            .is_none()
        {
            return Ok(None);
        }
        let mut playback = Self {
            quality,
            credits,
            events,
            token,
            pending: None,
            origin: None,
            paused: None,
            in_flight: false,
            animated: false,
            ended: false,
            last_presented: None,
        };
        playback.request();
        Ok(Some(playback))
    }
    fn request(&mut self) {
        if !self.in_flight && self.pending.is_none() && !self.ended && self.paused.is_none() {
            self.in_flight = self.credits.try_send(()).is_ok();
        }
    }
    pub(super) fn paused(&self) -> bool {
        self.paused.is_some()
    }
    pub(super) fn tick_needed(&self) -> bool {
        !self.ended && !self.paused()
    }
    pub(super) fn toggle(&mut self, now: Instant) {
        if self.ended {
            return;
        }
        if let Some(paused) = self.paused.take() {
            if let Some(origin) = self.origin {
                self.origin = origin.checked_add(now.saturating_duration_since(paused));
            }
            self.request();
        } else {
            self.paused = Some(now);
        }
    }
    pub(super) fn poll(&mut self, now: Instant) -> Option<Result<Option<Payload>, Error>> {
        if self.ended || self.paused() {
            return None;
        }
        if self.pending.is_none() {
            match self.events.try_recv() {
                Ok(Ok(event)) => {
                    self.pending = Some(event);
                    self.in_flight = false;
                }
                Ok(Err(error)) => {
                    self.ended = true;
                    return Some(Err(error));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.ended = true;
                    return Some(Err(Error::Failed));
                }
                Err(mpsc::TryRecvError::Empty) => return None,
            }
        }
        match self.pending.as_ref()? {
            Event::Frame { at_us, .. } | Event::End { at_us } => {
                let origin = *self.origin.get_or_insert(now);
                // Catch up after a busy UI without replaying stale presentations. The child
                // has already composed every dependent frame; only its output is discarded.
                // If decoding itself cannot catch up, still present progress after 250 ms:
                // dropping every late frame forever would freeze the poster on modest PCs.
                if matches!(self.pending, Some(Event::Frame { .. }))
                    && self.animated
                    && self.last_presented.is_some_and(|last| {
                        now.saturating_duration_since(last) < Duration::from_millis(250)
                    })
                    && now
                        .saturating_duration_since(origin)
                        .as_micros()
                        .saturating_sub(u128::from(*at_us))
                        > 250_000
                {
                    self.pending = None;
                    self.request();
                    return None;
                }
                if now.saturating_duration_since(origin) < Duration::from_micros(*at_us) {
                    return None;
                }
                if matches!(self.pending, Some(Event::Frame { .. }))
                    && self.last_presented.is_some_and(|last| {
                        now.saturating_duration_since(last)
                            < Duration::from_micros(self.quality.interval_us())
                    })
                {
                    return None;
                }
            }
            _ => {}
        }
        match self.pending.take()? {
            Event::Frame { image, .. } => {
                self.animated = true;
                self.last_presented = Some(now);
                self.request();
                Some(isolated::from_wire(image).map(Some))
            }
            Event::Static(response) => {
                self.ended = true;
                Some(isolated::from_wire(response).map(Some))
            }
            Event::End { .. } => {
                self.ended = true;
                Some(Ok(None))
            }
        }
    }
}
impl Drop for Playback {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::preview_wire::Response;
    fn playback() -> (Playback, SyncSender<Result<Event, Error>>, Receiver<()>) {
        let (credits, credit_rx) = mpsc::sync_channel(1);
        let (tx, events) = mpsc::sync_channel(1);
        (
            Playback {
                quality: Quality::default(),
                credits,
                events,
                token: CancellationToken::new(),
                pending: None,
                origin: None,
                paused: None,
                in_flight: true,
                animated: false,
                ended: false,
                last_presented: None,
            },
            tx,
            credit_rx,
        )
    }
    fn frame(at_us: u64) -> Event {
        Event::Frame {
            at_us,
            image: Response::Image {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            },
        }
    }
    #[test]
    fn clock_preserves_delays_pause_and_credit_bound() {
        let (mut a, tx, credits) = playback();
        let now = Instant::now();
        tx.send(Ok(frame(0))).unwrap();
        assert!(a.poll(now).is_some());
        credits.try_recv().unwrap();
        tx.send(Ok(frame(100_000))).unwrap();
        assert!(a.poll(now + Duration::from_millis(20)).is_none());
        a.toggle(now + Duration::from_millis(40));
        assert!(!a.tick_needed());
        assert!(a.poll(now + Duration::from_secs(15)).is_none());
        assert!(credits.try_recv().is_err());
        a.toggle(now + Duration::from_secs(15));
        assert!(
            credits.try_recv().is_err(),
            "resume must not add a credit behind a pending frame"
        );
        assert!(a.poll(now + Duration::from_millis(15059)).is_none());
        assert!(a.poll(now + Duration::from_millis(15060)).is_some());
        credits.try_recv().unwrap();
        tx.send(Ok(Event::End { at_us: 200_000 })).unwrap();
        assert!(a.poll(now + Duration::from_millis(15159)).is_none());
        assert!(matches!(
            a.poll(now + Duration::from_millis(15160)),
            Some(Ok(None))
        ));
        assert!(!a.tick_needed());
    }
    #[test]
    fn paused_full_queue_drop_cancels_and_generation_cannot_deliver_to_next() {
        let (mut a, tx, _credits) = playback();
        let token = a.token.clone();
        a.toggle(Instant::now());
        tx.try_send(Ok(frame(0))).unwrap();
        assert!(tx.try_send(Ok(frame(100_000))).is_err());
        drop(a);
        assert!(token.is_cancelled());
        assert!(tx.try_send(Ok(frame(200_000))).is_err());
        let (mut next, _tx, _credits) = playback();
        assert!(next.poll(Instant::now()).is_none());
    }
    #[test]
    fn static_result_and_error_stop_clock_instead_of_retrying() {
        let (mut a, tx, _credits) = playback();
        tx.send(Ok(Event::Static(Response::Image {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        })))
        .unwrap();
        assert!(a.poll(Instant::now()).is_some());
        assert!(!a.animated && !a.tick_needed());
        let (mut a, tx, _credits) = playback();
        drop(tx);
        assert!(matches!(a.poll(Instant::now()), Some(Err(Error::Failed))));
        assert!(!a.tick_needed());
    }
    #[test]
    fn overdue_presentations_are_dropped_without_resetting_clock() {
        let (mut a, tx, credits) = playback();
        let now = Instant::now();
        tx.send(Ok(frame(0))).unwrap();
        a.poll(now);
        credits.try_recv().unwrap();
        tx.send(Ok(frame(100_000))).unwrap();
        assert!(
            a.poll(now + Duration::from_secs(1)).is_some(),
            "slow decoder must still present progress"
        );
        credits.try_recv().unwrap();
        tx.send(Ok(frame(200_000))).unwrap();
        assert!(
            a.poll(now + Duration::from_millis(1100)).is_none(),
            "discard overdue output during catch-up"
        );
        credits.try_recv().unwrap();
        assert_eq!(a.origin, Some(now));
    }
    #[test]
    fn animation_lifecycle_cancel_hidden_and_a_b_a_preserve_generation() {
        for hidden in [false, true] {
            let (a, tx, _credits) = playback();
            let token = a.token.clone();
            let now = Instant::now();
            let mut state = PreviewState::new();
            state.set_wanted(Some("a.webp".into()), now);
            state.loaded = state.wanted.clone();
            state.animation = Some(a);
            assert!(
                !state.busy(),
                "playing must not keep full workspace tick alive"
            );
            assert!(state.can_cancel(now));
            if hidden {
                state.set_visible(false, now);
            } else {
                assert!(state.cancel());
            }
            assert!(token.is_cancelled());
            assert!(tx.try_send(Ok(frame(0))).is_err());
            state.set_visible(true, now);
            assert_eq!(state.should_start(now + PREVIEW_DEBOUNCE), hidden);
            state.set_wanted(Some("b.webp".into()), now);
            state.set_wanted(Some("a.webp".into()), now);
            assert!(state.last_view().is_none());
        }
    }
    #[test]
    fn disabling_current_image_rule_retires_animation_immediately() {
        let (a, tx, _credits) = playback();
        let token = a.token.clone();
        let mut state = PreviewState::new();
        state.set_wanted(Some("a.webp".into()), Instant::now());
        state.animation = Some(a);
        let mut rules = state.rules.clone();
        rules.iter_mut().find(|r| r.ext == "webp").unwrap().enabled = false;
        state.set_rules(rules);
        assert!(token.is_cancelled());
        assert!(state.animation.is_none());
        assert!(tx.send(Ok(frame(0))).is_err());
        assert_eq!(
            preview::classify_rules(state.wanted.as_ref().unwrap(), &state.rules),
            PreviewKind::None
        );
    }
}
