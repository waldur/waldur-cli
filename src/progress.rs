//! Hand-written: an interactive-only progress spinner for the long, silent
//! wait while a marketplace order is polled to completion (`order::poll_order`).
//!
//! It writes to **stderr** (stdout stays clean for scripts/agents) and only
//! when stderr is a real terminal *and* `--debug` is off -- under `--debug`
//! the request trace already shows each poll, and a `\r`-redrawing spinner on
//! the same stream would shred it. Non-interactive runs (pipes, redirects, CI,
//! agents) get nothing, so their stderr stays byte-clean.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Set once in `main` from `--debug`. `poll_order` is reached through generated
/// command code, so a process-global is cheaper than threading a flag through
/// every generated order arm for something only this hand-written path reads.
static DEBUG: AtomicBool = AtomicBool::new(false);

pub fn set_debug(on: bool) {
    DEBUG.store(on, Ordering::Relaxed);
}

fn debug_enabled() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

/// A single-line spinner redrawn in place on stderr. Disabled (every method a
/// no-op) unless stderr is a TTY and `--debug` is off, so callers don't need to
/// branch.
pub struct Spinner {
    enabled: bool,
    frames: &'static [&'static str],
    i: usize,
    label: &'static str,
}

impl Spinner {
    pub fn new(label: &'static str) -> Self {
        let enabled = std::io::stderr().is_terminal() && !debug_enabled();
        Self {
            enabled,
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            i: 0,
            label,
        }
    }

    /// Advance one frame, showing elapsed seconds and the last-known order
    /// state (e.g. `⠹ Provisioning… 42s (executing)`). Called far more often
    /// than the order is polled, so the animation stays lively between polls.
    pub fn tick(&mut self, elapsed: Duration, state: &str) {
        if !self.enabled {
            return;
        }
        let frame = self.frames[self.i % self.frames.len()];
        self.i += 1;
        // \r returns to column 0; \x1b[2K clears the whole line so a shorter
        // frame doesn't leave stale characters from a longer previous one.
        eprint!(
            "\r\x1b[2K{frame} {}… {}s ({state})",
            self.label,
            elapsed.as_secs()
        );
        let _ = std::io::stderr().flush();
    }

    /// Erase the spinner line so the final stdout result or an error message
    /// isn't preceded by leftover spinner text. Safe to call when disabled.
    pub fn clear(&self) {
        if !self.enabled {
            return;
        }
        eprint!("\r\x1b[2K");
        let _ = std::io::stderr().flush();
    }
}
