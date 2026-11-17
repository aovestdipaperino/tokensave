//! Explicit, separately consented sharing of the sanitized export.
//!
//! This is the only profiling module allowed to perform network I/O.
//! The shipping destination remains unset until the maintainer publishes the
//! endpoint, access, retention and deletion policy. All runtime paths fail closed.

use std::path::Path;
use std::time::Duration;

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use super::{export, storage, Mode};

pub const INTERVAL_SECS: u64 = 24 * 3600;
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(2);

/// Maintainer-approved destination and policy shown before consent.
#[derive(Debug, Clone, Copy)]
pub struct Destination {
    pub url: &'static str,
    pub access: &'static str,
    pub retention: &'static str,
    pub deletion: &'static str,
}

impl Destination {
    pub fn ready(self) -> bool {
        self.url.starts_with("https://")
            && !self.access.trim().is_empty()
            && !self.retention.trim().is_empty()
            && !self.deletion.trim().is_empty()
    }
}

/// Intentionally unset. No environment variable or config value can redirect uploads.
pub const DESTINATION: Option<Destination> = None;

/// The override can only disable sharing. Unknown values also fail closed.
pub fn override_allows(value: Option<&str>) -> bool {
    value.is_none_or(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub fn environment_allows() -> bool {
    override_allows(std::env::var("TOKENSAVE_PROFILE_SHARE").ok().as_deref())
}

/// Same serializer, window and trailing newline as `profile export` and preview.
pub fn payload(dir: &Path, now: u64) -> String {
    let merged = storage::load(dir, now.saturating_sub(7 * INTERVAL_SECS) / 3600);
    let data = export::sanitized(&merged, 7 * 24, &export::known_tools());
    format!("{}\n", export::to_json(&data))
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct State {
    pub last_attempt: u64,
    pub last_upload: u64,
}

pub fn state(dir: &Path) -> Option<State> {
    let path = dir.parent()?.join("profile-sharing.json");
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).ok(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(State::default()),
        Err(_) => None,
    }
}

fn due(state: State, now: u64) -> bool {
    state.last_attempt == 0 || now.saturating_sub(state.last_attempt) >= INTERVAL_SECS
}

trait Transport {
    fn send(&self, destination: Destination, body: &str) -> bool;
}

struct Https;
impl Transport for Https {
    fn send(&self, destination: Destination, body: &str) -> bool {
        crate::cloud::agent_with_timeout(TIMEOUT)
            .post(destination.url)
            .config()
            .max_redirects(0)
            .build()
            .header("Content-Type", "application/json")
            .send(body)
            .is_ok_and(|response| response.status().is_success())
    }
}

/// Called on the existing flush thread, after local persistence. No new task,
/// queue or retry loop is created. Nothing is retained after the attempt.
pub(crate) fn after_flush(dir: &Path, now: u64, scope: &super::local::Scope) {
    attempt(
        dir,
        now,
        DESTINATION,
        environment_allows(),
        || scope.settings().mode != Mode::Off && scope.sharing_consent(),
        &Https,
    );
}

fn attempt(
    dir: &Path,
    now: u64,
    destination: Option<Destination>,
    env_allows: bool,
    consent: impl Fn() -> bool,
    transport: &impl Transport,
) {
    // Gate before file creation, profile loading, serialization or transport construction.
    let Some(destination) = destination.filter(|d| d.ready()) else {
        return;
    };
    if !env_allows || !consent() {
        return;
    }
    let Some(parent) = dir.parent() else { return };
    let Ok(lock) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(parent.join("profile-sharing.lock"))
    else {
        return;
    };
    // One owner across processes; no waiting if another process is uploading.
    if lock.try_lock_exclusive().is_err() {
        return;
    }
    let Some(mut state) = state(dir) else { return };
    if !due(state, now) || !consent() {
        return;
    }
    let body = payload(dir, now);
    if body.len() > MAX_PAYLOAD_BYTES {
        return;
    }
    // Claim the day before the request, including failures. A crash cannot cause
    // repeated uploads. Clock rollback suppresses attempts until time catches up.
    state.last_attempt = now;
    let path = parent.join("profile-sharing.json");
    let Ok(text) = serde_json::to_string(&state) else {
        return;
    };
    if !crate::user_config::write_atomic(&path, &text) || !consent() {
        return;
    }
    if transport.send(destination, &body) {
        state.last_upload = now;
        if let Ok(text) = serde_json::to_string(&state) {
            let _ = crate::user_config::write_atomic(&path, &text);
        }
    }
    // File closes and releases the advisory lock on every exit path.
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    const TEST_DESTINATION: Destination = Destination {
        url: "https://example.invalid/profile",
        access: "maintainers",
        retention: "30 days",
        deletion: "contact maintainer",
    };
    #[derive(Default)]
    struct Mock {
        bodies: RefCell<Vec<String>>,
        fail: Cell<bool>,
    }
    impl Transport for Mock {
        fn send(&self, _: Destination, body: &str) -> bool {
            self.bodies.borrow_mut().push(body.to_string());
            !self.fail.get()
        }
    }

    #[test]
    fn gates_do_not_create_files_or_call_transport() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profile");
        let mock = Mock::default();
        attempt(&dir, 100_000, None, true, || true, &mock);
        attempt(&dir, 100_000, Some(TEST_DESTINATION), false, || true, &mock);
        attempt(&dir, 100_000, Some(TEST_DESTINATION), true, || false, &mock);
        attempt(
            &dir,
            100_000,
            Some(Destination {
                url: "http://example.invalid",
                ..TEST_DESTINATION
            }),
            true,
            || true,
            &mock,
        );
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
        assert!(mock.bodies.borrow().is_empty());
    }

    #[test]
    fn exact_export_payload_and_daily_attempt_limit_survive_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profile");
        let mock = Mock::default();
        attempt(&dir, 100_000, Some(TEST_DESTINATION), true, || true, &mock);
        assert_eq!(mock.bodies.borrow()[0], payload(&dir, 100_000));
        assert_eq!(state(&dir).unwrap().last_upload, 100_000);
        attempt(&dir, 100_001, Some(TEST_DESTINATION), true, || true, &mock);
        assert_eq!(mock.bodies.borrow().len(), 1);
        let restarted = Mock::default();
        restarted.fail.set(true);
        attempt(
            &dir,
            100_000 + INTERVAL_SECS,
            Some(TEST_DESTINATION),
            true,
            || true,
            &restarted,
        );
        attempt(
            &dir,
            100_001 + INTERVAL_SECS,
            Some(TEST_DESTINATION),
            true,
            || true,
            &restarted,
        );
        assert_eq!(restarted.bodies.borrow().len(), 1);
        assert_eq!(state(&dir).unwrap().last_upload, 100_000);
    }

    #[test]
    fn rechecks_revocation_before_transport() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profile");
        let checks = Cell::new(0);
        let mock = Mock::default();
        attempt(
            &dir,
            100_000,
            Some(TEST_DESTINATION),
            true,
            || {
                checks.set(checks.get() + 1);
                checks.get() < 3
            },
            &mock,
        );
        assert!(mock.bodies.borrow().is_empty());
    }

    #[test]
    fn competing_process_lock_and_corrupt_state_block_transport() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profile");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(tmp.path().join("profile-sharing.lock"))
            .unwrap();
        lock.lock_exclusive().unwrap();
        let mock = Mock::default();
        attempt(&dir, 100_000, Some(TEST_DESTINATION), true, || true, &mock);
        assert!(mock.bodies.borrow().is_empty());
        drop(lock);
        std::fs::write(tmp.path().join("profile-sharing.json"), "broken").unwrap();
        attempt(&dir, 100_000, Some(TEST_DESTINATION), true, || true, &mock);
        assert!(mock.bodies.borrow().is_empty());
    }

    #[test]
    fn oversized_export_is_skipped_before_attempt_is_claimed() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("profile");
        let mut series = super::super::SeriesSnapshot::default();
        for index in 0..super::super::histogram::BUCKETS {
            series
                .histogram
                .record(super::super::histogram::bucket_bounds(index).0);
        }
        let snapshot = (0..super::super::Op::COUNT)
            .map(|index| {
                (
                    super::super::Op::from_index(index)
                        .unwrap()
                        .name()
                        .to_string(),
                    series.clone(),
                )
            })
            .collect();
        let mut store = storage::Store::new(dir.clone(), 14, 32 * 1024 * 1024);
        store.flush(&snapshot, 100_000).unwrap();
        assert!(payload(&dir, 100_000).len() > MAX_PAYLOAD_BYTES);
        let mock = Mock::default();
        attempt(&dir, 100_000, Some(TEST_DESTINATION), true, || true, &mock);
        assert!(mock.bodies.borrow().is_empty());
        assert_eq!(state(&dir).unwrap().last_attempt, 0);
    }

    #[test]
    fn clock_rollback_does_not_reset_daily_limit() {
        assert!(!due(
            State {
                last_attempt: 100_000,
                last_upload: 100_000
            },
            99_000
        ));
        assert!(!due(
            State {
                last_attempt: 100_000,
                last_upload: 0
            },
            100_000 + INTERVAL_SECS - 1
        ));
    }

    #[test]
    fn override_never_enables_consent() {
        assert!(override_allows(None));
        for v in ["0", "false", "no", "off", "", "unexpected"] {
            assert!(!override_allows(Some(v)));
        }
        assert!(override_allows(Some("1")));
        assert!(DESTINATION.is_none());
    }
}
