//! Cooperative cancellation for long-running work (#450).
//!
//! A sync is not interruptible by a signal on its own. The MCP run loop holds
//! a SIGTERM stream for the life of the loop, but that stream is only polled
//! while the loop waits for the next request, so a signal arriving *during*
//! `handle_request` is retained and observed only after the request finishes.
//! For a request that is a whole-graph sync of a large tree, "after the
//! request finishes" can be minutes, and until then `kill` looks ignored — the
//! behaviour reported in #450.
//!
//! The fix is a process-global flag that a signal handler sets immediately and
//! that long-running work polls at points where abandoning it is safe. Sync is
//! already built for this: `try_acquire_sync_lock` is an RAII guard, and the
//! dirty sentinel is written at the start and cleared only on success. Leaving
//! early through `Err` therefore releases the lock and leaves the index
//! correctly marked dirty, so the next sync redoes the abandoned work.
//!
//! The flag is deliberately global rather than threaded through every call:
//! the work it interrupts runs across a rayon pool and several async layers,
//! and a parameter would have to reach all of them to be useful.

use std::sync::atomic::{AtomicBool, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);

/// Wakes a task parked on something that will never complete — the MCP run
/// loop's `read_line`, whose blocking stdin read cannot be cancelled and never
/// returns while a supervisor holds our stdin open.
fn gate() -> &'static tokio::sync::Notify {
    static GATE: std::sync::OnceLock<tokio::sync::Notify> = std::sync::OnceLock::new();
    GATE.get_or_init(tokio::sync::Notify::new)
}

/// Marks the process as shutting down. Idempotent, and safe to call from a
/// signal-handling task.
pub fn request() {
    CANCELLED.store(true, Ordering::SeqCst);
    // `notify_one` stores a permit, so a waiter that has not parked yet still
    // sees this. Without it a shutdown requested while the run loop is between
    // iterations would be missed.
    gate().notify_one();
}

/// Has a shutdown been requested?
///
/// Cheap enough to poll per file in an extraction loop: a relaxed-ordering
/// atomic load against work that reads and parses a source file.
#[must_use]
pub fn is_requested() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

/// Resolves once a shutdown has been requested.
///
/// For a `select!` arm alongside work that cannot itself be cancelled.
pub async fn cancelled() {
    // Re-check rather than trusting the wake-up. `notify_one` leaves a stored
    // permit when nobody is parked, and `reset` cannot take it back, so a
    // notification can outlive the request that caused it. Treating any wake
    // as a cancellation would then stop the next run loop the moment it
    // started.
    loop {
        if is_requested() {
            return;
        }
        gate().notified().await;
    }
}

/// Clears the flag. For tests, and for a caller that deliberately runs more
/// work after observing a cancellation.
pub fn reset() {
    CANCELLED.store(false, Ordering::SeqCst);
}

/// The error a cancelled operation returns.
///
/// Worded for the user who pressed Ctrl-C or sent the `kill`. It deliberately
/// does not promise the index is unchanged: a full `sync --force` clears the
/// graph before extracting, so an interruption after that point leaves a real
/// gap. What it can promise is that no partial extraction was committed and
/// that the dirty sentinel survives, so the next sync redoes the work.
#[must_use]
pub fn interrupted_error(what: &str) -> crate::errors::TokenSaveError {
    crate::errors::TokenSaveError::Config {
        message: format!(
            "{what} interrupted by shutdown signal — no partial results were committed, \
             and the index is left marked stale; run `tokensave sync` to finish it"
        ),
    }
}

/// Returns `Err` when a shutdown has been requested. Call at points where the
/// work in flight can be abandoned without leaving inconsistent state.
pub fn check(what: &str) -> crate::errors::Result<()> {
    if is_requested() {
        return Err(interrupted_error(what));
    }
    Ok(())
}

/// [`check`], for a point past which rows have already been written.
///
/// The insert phases cannot be unwound — they are not one transaction — so
/// stopping there leaves some files updated and others not. That is the state
/// a crash leaves too, and the dirty sentinel already covers it; what changes
/// is only what the user is told, which must not claim nothing was written.
pub fn check_partial(what: &str) -> crate::errors::Result<()> {
    if is_requested() {
        return Err(crate::errors::TokenSaveError::Config {
            message: format!(
                "{what} interrupted by shutdown signal partway through writing — the index \
                 is partially updated and left marked stale; run `tokensave sync` to finish it"
            ),
        });
    }
    Ok(())
}

/// Installs the process-wide signal handlers that set the flag.
///
/// Spawns one task that lives for the process. Unlike a stream polled inside a
/// `select!`, this observes a signal the moment it is delivered, whatever the
/// main task is doing — which is the whole point for a signal that arrives
/// mid-sync.
///
/// Safe to call more than once; only the first call installs anything.
pub fn install_signal_handlers() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tokio::spawn(async {
            #[cfg(unix)]
            {
                // Nothing to fall back to, and a server that cannot register
                // a handler must still serve.
                let Ok(mut sigterm) =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                else {
                    return;
                };
                tokio::select! {
                    _ = sigterm.recv() => request(),
                    _ = tokio::signal::ctrl_c() => request(),
                }
            }
            #[cfg(not(unix))]
            {
                if tokio::signal::ctrl_c().await.is_ok() {
                    request();
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_passes_until_a_shutdown_is_requested() {
        reset();
        assert!(check("sync").is_ok());
        request();
        match check("sync") {
            Ok(()) => panic!("must fail once a shutdown is requested"),
            Err(err) => assert!(
                err.to_string().contains("interrupted by shutdown signal"),
                "got: {err}"
            ),
        }
        reset();
        assert!(check("sync").is_ok(), "reset must clear the flag");
    }
}
