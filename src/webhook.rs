use crate::audio_monitor::AudioState;
use log::{error, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub fn start_webhook(
    url_free: Arc<str>,
    url_speaker: Arc<str>,
    url_busy: Arc<str>,
    url_calmdown: Arc<str>,
    enabled: Arc<AtomicBool>,
    calmdown_timeout: u64,
    rx: crossbeam_channel::Receiver<AudioState>,
    running: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let mut last_state: Option<AudioState> = None;
        let mut debounce: Option<Instant> = None;
        // Calmdown: when did we enter Free, and have we already fired it?
        let mut free_since: Option<Instant> = None;
        let mut calmdown_fired = false;

        while running.load(Ordering::Relaxed) {
            let state = match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(s) => s,
                Err(_) => {
                    // Debounced state send
                    if let (Some(ts), Some(prev)) = (debounce, last_state) {
                        if ts.elapsed() >= Duration::from_secs(2) {
                            debounce = None;
                            if enabled.load(Ordering::Relaxed) {
                                let url = pick_url(&url_free, &url_speaker, &url_busy, prev);
                                send(url, prev);
                            }
                        }
                    }

                    // Calmdown check — only fires when Free has been stable
                    // for `calmdown_secs` and no other state interrupted.
                    if enabled.load(Ordering::Relaxed) && calmdown_timeout > 0 {
                        if let Some(start) = free_since {
                            if !calmdown_fired && start.elapsed() >= Duration::from_secs(calmdown_timeout) {
                                calmdown_fired = true;
                                info!("Calmdown timer expired — firing calmdown webhook");
                                send(&url_calmdown, AudioState::Free);
                            }
                        }
                    }
                    continue;
                }
            };

            // New state arrived
            last_state = Some(state);
            debounce = Some(Instant::now());

            // Calmdown: reset on every state change
            match state {
                AudioState::Free => {
                    free_since = Some(Instant::now());
                    calmdown_fired = false;
                }
                _ => {
                    free_since = None;
                    calmdown_fired = false;
                }
            }
        }
    });
}

fn pick_url<'a>(free: &'a str, speaker: &'a str, busy: &'a str, state: AudioState) -> &'a str {
    match state {
        AudioState::Free => free,
        AudioState::SpeakerActive => speaker,
        AudioState::Busy => busy,
    }
}

/// Fire a one-shot calmdown to the given URL (called on quit / sleep).
pub fn fire_calmdown(url: &str) {
    info!("Calmdown (one-shot) → {url}");
    match ureq::post(url).send_empty() {
        Ok(resp) => {
            if resp.status().is_success() {
                info!("Calmdown OK");
            }
        }
        Err(_) => {} // best-effort
    }
}

fn send(url: &str, state: AudioState) {
    let label = match state {
        AudioState::Free => "free",
        AudioState::SpeakerActive => "speaker",
        AudioState::Busy => "busy",
    };
    info!("Webhook → {url}  state={label}");

    match ureq::post(url).send_empty() {
        Ok(resp) => {
            let s = resp.status();
            if s.is_success() {
                info!("Webhook OK ({s})");
            } else {
                error!("Webhook returned HTTP {s}");
            }
        }
        Err(ureq::Error::StatusCode(s)) => {
            error!("Webhook request failed: HTTP {s}");
        }
        Err(e) => {
            error!("Webhook request failed: {e}");
        }
    }
}
