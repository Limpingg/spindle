//! Spindle: where the seconds go, at INFO level in the ordinary log, so a
//! log from Nick's PC answers "what is slow" without a verbose run.
//!
//! Two kinds of lines:
//! - `startup: <milestone> at <ms> ms`, once per milestone, measured from
//!   the process start: first frame, signed in, playback session ready,
//!   playlists listed, home loaded, friends loaded.
//! - `play: sound after <ms> ms (<what>)` for every play the user starts:
//!   from the click to the first second of audio reported by the player.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static STARTED: OnceLock<Instant> = OnceLock::new();
static SEEN: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
static PLAY: Mutex<Option<(Instant, String)>> = Mutex::new(None);

/// Marks the process start. Called first thing; later calls change nothing.
pub fn start() {
    STARTED.get_or_init(Instant::now);
}

/// Milliseconds since [`start`].
pub fn since_start_ms() -> u128 {
    STARTED.get_or_init(Instant::now).elapsed().as_millis()
}

/// Logs a startup milestone the first time it is reached.
pub fn milestone(name: &'static str) {
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let first = seen
        .lock()
        .map(|mut seen| seen.insert(name))
        .unwrap_or(false);
    if first {
        log::info!("startup: {name} at {} ms", since_start_ms());
    }
}

/// The user asked for `what` to play; the clock starts.
pub fn play_requested(what: String) {
    if let Ok(mut play) = PLAY.lock() {
        *play = Some((Instant::now(), what));
    }
}

/// The player reports sound. Logs the wait since the request, once.
pub fn playing() {
    if let Ok(mut play) = PLAY.lock()
        && let Some((at, what)) = play.take()
    {
        log::info!("play: sound after {} ms ({what})", at.elapsed().as_millis());
    }
}

/// What the clock is running for, if a play is still waiting for sound.
#[cfg(test)]
pub fn pending_play() -> Option<String> {
    PLAY.lock().ok().and_then(|play| play.as_ref().map(|(_, what)| what.clone()))
}

/// The request was dropped or superseded: nothing to time.
pub fn play_cancelled() {
    if let Ok(mut play) = PLAY.lock() {
        *play = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_milestone_is_counted_once_and_a_play_is_timed_once() {
        start();
        milestone("test milestone");
        milestone("test milestone");
        assert!(since_start_ms() < 60_000);
        play_requested("spotify:track:x".into());
        playing();
        assert!(PLAY.lock().unwrap().is_none());
        play_cancelled();
    }
}
