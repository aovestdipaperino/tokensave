//! Opt-in filesystem watcher for `tokensave serve` (#937).
//!
//! Off by default. When `filesystem_watch_enabled = true` in
//! `~/.tokensave/config.toml`, or `serve --watch`, asks for it, the server
//! registers native file notifications (inotify, `FSEvents`,
//! `ReadDirectoryChangesW`, kqueue) through the `notify` crate and applies
//! changes through the existing incremental sync. Nothing here replaces the
//! on-demand staleness check: the startup catch-up sync and the 30 s check at
//! the top of every `tools/call` keep running, so a notification that never
//! arrives is still picked up the way it was before the watcher existed.
//!
//! Resource bounds, all fixed:
//!
//! - the notify callback hands events to a bounded channel
//!   ([`CHANNEL_CAPACITY`]) with `try_send`; a full channel drops the event
//!   and flags a reconciliation instead of growing a queue,
//! - one debounced batch holds at most [`MAX_PENDING_PATHS`] distinct paths;
//!   past that it stops collecting and flags a reconciliation,
//! - a per-directory watcher registers at most [`MAX_WATCHED_DIRS`]
//!   directories, and refuses to start rather than watch part of a tree,
//! - a reconciliation is the existing bounded scan
//!   ([`crate::tokensave::TokenSave::find_stale_files_bounded`]), run at most
//!   once per [`MIN_RECONCILE_INTERVAL`].
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::tokensave::{is_transient_file_name, WatchedChange};

/// Debounce used when `watcher_debounce` is missing or unparseable.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(2);
/// Shortest accepted debounce; anything lower is raised to it.
pub const MIN_DEBOUNCE: Duration = Duration::from_millis(50);
/// Longest accepted debounce; anything higher is lowered to it.
pub const MAX_DEBOUNCE: Duration = Duration::from_secs(60);
/// Capacity of the channel between the notify callback and the batcher.
pub const CHANNEL_CAPACITY: usize = 1024;
/// Most distinct paths one batch collects before it becomes a
/// reconciliation.
pub const MAX_PENDING_PATHS: usize = 4096;
/// Most directories a per-directory watcher registers.
pub const MAX_WATCHED_DIRS: usize = 16_384;
/// Shortest interval between two watcher-triggered reconciliation scans.
pub const MIN_RECONCILE_INTERVAL: Duration = Duration::from_secs(10);
/// How long shutdown waits for the watcher task to finish its batch.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// Most reconciliation reasons kept for one batch's log line.
const MAX_REASONS: usize = 4;

/// The watcher's tunables, resolved from the user configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchSettings {
    /// Quiet period a burst of events must end with before it is applied.
    pub debounce: Duration,
}

impl WatchSettings {
    /// Settings from the `watcher_debounce` string (`"2s"`, `"500ms"`,
    /// `"1m"`). An unparseable value falls back to [`DEFAULT_DEBOUNCE`] with
    /// a warning; an out-of-range one is clamped to
    /// [`MIN_DEBOUNCE`]..=[`MAX_DEBOUNCE`].
    pub fn from_debounce_setting(raw: &str) -> Self {
        let debounce = if let Some(parsed) = crate::user_config::parse_duration(raw) {
            parsed.clamp(MIN_DEBOUNCE, MAX_DEBOUNCE)
        } else {
            eprintln!(
                "[tokensave] filesystem watch: watcher_debounce = {raw:?} is not a duration \
                 such as \"2s\" or \"500ms\"; using {}ms",
                DEFAULT_DEBOUNCE.as_millis()
            );
            DEFAULT_DEBOUNCE
        };
        Self { debounce }
    }

    /// Longest a batch waits under a continuous stream of events before it
    /// is applied anyway, so a busy tree cannot postpone indexing forever.
    pub fn max_batch_wait(&self) -> Duration {
        self.debounce
            .saturating_mul(5)
            .clamp(Duration::from_secs(5), MAX_DEBOUNCE)
    }
}

/// How the watcher registers the project with the operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchMode {
    /// One recursive watch on the project root. Used where the native API
    /// is recursive by design (`FSEvents`, `ReadDirectoryChangesW`), so the
    /// operating system holds one handle however large the tree is; events
    /// from ignored directories still arrive and are filtered here.
    RecursiveRoot,
    /// One non-recursive watch per directory the scan walk enters. Used
    /// where the native API watches single directories (inotify, kqueue),
    /// so ignored and excluded directories never consume a watch.
    PerDirectory,
}

impl WatchMode {
    /// The mode for the platform this binary was built for.
    pub fn for_platform() -> Self {
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            Self::RecursiveRoot
        } else {
            Self::PerDirectory
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::RecursiveRoot => "recursive-root",
            Self::PerDirectory => "per-directory",
        }
    }
}

/// A message from the notify callback to the batcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchMsg {
    /// Paths that changed.
    Changes(Vec<WatchedChange>),
    /// The event stream is no longer trustworthy (an error, or the platform
    /// reported dropped events); reconcile with a scan.
    Rescan(String),
}

/// One debounced batch, handed to the code that applies it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WatchBatch {
    /// Distinct changed paths; a path is structural if any of its events was.
    pub changes: Vec<WatchedChange>,
    /// Whether the batch must be reconciled with a scan.
    pub rescan: bool,
    /// Why, when [`Self::rescan`] is set.
    pub reasons: Vec<String>,
}

/// What applying a batch asks of the watcher loop.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WatchOutcome {
    /// Directories a per-directory watcher should start watching.
    pub new_dirs: Vec<PathBuf>,
    /// A reconciliation that could not run yet; retry after this long even
    /// if no further event arrives.
    pub retry_rescan_after: Option<Duration>,
    /// A reconciliation ran, so any retry still scheduled is obsolete.
    pub reconciled: bool,
}

/// Counters and state reported under `filesystem_watch` in
/// `tokensave_status`, so watcher failures are observable without stderr.
#[derive(Debug, Default)]
pub struct WatchStats {
    state: Mutex<Option<&'static str>>,
    mode: Mutex<Option<&'static str>>,
    debounce_ms: AtomicU64,
    watched_dirs: AtomicU64,
    batches: AtomicU64,
    changed_paths: AtomicU64,
    reconciliations: AtomicU64,
    dropped_events: AtomicU64,
    errors: AtomicU64,
    last_error: Mutex<Option<String>>,
    last_reconcile: Mutex<Option<Instant>>,
    drift_reported: AtomicBool,
}

impl WatchStats {
    fn set_state(&self, state: &'static str) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = Some(state);
        }
    }

    /// The current state: `off`, `starting`, `active`, `failed` or
    /// `stopped`.
    pub fn state(&self) -> &'static str {
        self.state.lock().ok().and_then(|g| *g).unwrap_or("off")
    }

    /// Records an error, logs it, and keeps it as `last_error`.
    pub(crate) fn record_error(&self, message: &str) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        eprintln!("[tokensave] filesystem watch: {message}");
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(message.to_string());
        }
    }

    pub(crate) fn record_batch(&self, paths: usize) {
        self.batches.fetch_add(1, Ordering::Relaxed);
        self.changed_paths
            .fetch_add(u64::try_from(paths).unwrap_or(u64::MAX), Ordering::Relaxed);
    }

    /// Whether a reconciliation may run now; if not, how long until it may.
    pub(crate) fn reconcile_wait(&self) -> Option<Duration> {
        let last = self.last_reconcile.lock().ok().and_then(|g| *g)?;
        MIN_RECONCILE_INTERVAL.checked_sub(last.elapsed())
    }

    pub(crate) fn record_reconcile(&self) {
        self.reconciliations.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut guard) = self.last_reconcile.lock() {
            *guard = Some(Instant::now());
        }
    }

    /// Logs a branch drift once per drift episode rather than once per batch.
    pub(crate) fn report_drift(&self, message: &str) {
        if !self.drift_reported.swap(true, Ordering::Relaxed) {
            eprintln!("[tokensave] filesystem watch paused: {message}");
        }
    }

    pub(crate) fn clear_drift(&self) {
        self.drift_reported.store(false, Ordering::Relaxed);
    }

    /// The `filesystem_watch` object of `tokensave_status`.
    pub fn to_json(&self) -> Value {
        let state = self.state();
        let mut value = json!({ "state": state });
        if state == "off" {
            return value;
        }
        value["mode"] = json!(self.mode.lock().ok().and_then(|g| *g));
        value["debounce_ms"] = json!(self.debounce_ms.load(Ordering::Relaxed));
        value["watched_directories"] = json!(self.watched_dirs.load(Ordering::Relaxed));
        value["batches"] = json!(self.batches.load(Ordering::Relaxed));
        value["changed_paths"] = json!(self.changed_paths.load(Ordering::Relaxed));
        value["reconciliations"] = json!(self.reconciliations.load(Ordering::Relaxed));
        value["dropped_events"] = json!(self.dropped_events.load(Ordering::Relaxed));
        value["errors"] = json!(self.errors.load(Ordering::Relaxed));
        value["last_error"] = json!(self.last_error.lock().ok().and_then(|g| g.clone()));
        value
    }
}

/// Whether an event of `kind` matters, and if so whether it is structural
/// (creates, removes or renames a path) rather than a write.
pub fn classify_event(kind: &EventKind) -> Option<bool> {
    match kind {
        // Reads and closes change nothing on disk.
        EventKind::Access(_) => None,
        EventKind::Modify(ModifyKind::Name(_))
        | EventKind::Create(_)
        | EventKind::Remove(_)
        | EventKind::Any
        | EventKind::Other => Some(true),
        EventKind::Modify(_) => Some(false),
    }
}

/// Whether a path from a notification is never worth a batch slot: inside
/// `.git` or `TokenSave`'s state directory, or an editor's transient file.
/// The precise scoping happens later against the project's indexing rules;
/// this only keeps obvious noise, and `TokenSave`'s own database writes, out
/// of the bounded queue.
pub fn is_noise_path(path: &Path) -> bool {
    if path
        .components()
        .any(|c| matches!(c.as_os_str().to_str(), Some(".git" | ".tokensave")))
    {
        return true;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_transient_file_name)
}

/// Converts one notify callback result into a message for the batcher, or
/// `None` when it carries nothing worth applying.
pub fn event_to_message(result: notify::Result<notify::Event>) -> Option<WatchMsg> {
    let event = match result {
        Ok(event) => event,
        Err(error) => return Some(WatchMsg::Rescan(format!("watcher error: {error}"))),
    };
    if event.need_rescan() {
        return Some(WatchMsg::Rescan(
            "the operating system reported dropped events".to_string(),
        ));
    }
    let structural = classify_event(&event.kind)?;
    let changes: Vec<WatchedChange> = event
        .paths
        .into_iter()
        .filter(|path| !is_noise_path(path))
        .map(|path| WatchedChange { path, structural })
        .collect();
    (!changes.is_empty()).then_some(WatchMsg::Changes(changes))
}

/// The paths collected for one batch, bounded by [`MAX_PENDING_PATHS`].
#[derive(Debug, Default)]
struct Pending {
    paths: HashMap<PathBuf, bool>,
    rescan: bool,
    reasons: Vec<String>,
}

impl Pending {
    fn add_reason(&mut self, reason: String) {
        self.rescan = true;
        if self.reasons.len() < MAX_REASONS && !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }

    fn absorb(&mut self, msg: WatchMsg) {
        match msg {
            WatchMsg::Rescan(reason) => self.add_reason(reason),
            WatchMsg::Changes(changes) => {
                if self.rescan && self.paths.len() >= MAX_PENDING_PATHS {
                    return;
                }
                for change in changes {
                    if let Some(structural) = self.paths.get_mut(&change.path) {
                        *structural |= change.structural;
                    } else if self.paths.len() < MAX_PENDING_PATHS {
                        self.paths.insert(change.path, change.structural);
                    } else {
                        self.add_reason(format!(
                            "more than {MAX_PENDING_PATHS} paths changed in one batch"
                        ));
                        return;
                    }
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.paths.is_empty() && !self.rescan
    }

    fn into_batch(self) -> WatchBatch {
        let mut changes: Vec<WatchedChange> = self
            .paths
            .into_iter()
            .map(|(path, structural)| WatchedChange { path, structural })
            .collect();
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        WatchBatch {
            changes,
            rescan: self.rescan,
            reasons: self.reasons,
        }
    }
}

/// Collects messages into debounced batches and applies them until
/// `cancel` fires or every sender is gone.
///
/// A batch is applied once no message has arrived for `settings.debounce`,
/// or once it is [`WatchSettings::max_batch_wait`] old. `overflow` is set by
/// the callback when the channel was full; it turns the next batch into a
/// reconciliation. `apply` runs the batch to completion — it is not
/// cancelled part-way through a sync — and `register` receives the
/// directories a per-directory watcher should add.
pub async fn run_watch_loop<A, Fut, R>(
    mut rx: mpsc::Receiver<WatchMsg>,
    overflow: Arc<AtomicBool>,
    settings: WatchSettings,
    cancel: CancellationToken,
    mut apply: A,
    mut register: R,
) where
    A: FnMut(WatchBatch) -> Fut,
    Fut: Future<Output = WatchOutcome>,
    R: FnMut(Vec<PathBuf>),
{
    let mut retry_at: Option<Instant> = None;
    loop {
        let mut pending = Pending::default();
        // Wait for the first message, or for a deferred reconciliation.
        let first = tokio::select! {
            () = cancel.cancelled() => return,
            msg = rx.recv() => match msg {
                Some(msg) => Some(msg),
                None => return,
            },
            () = sleep_until_opt(retry_at) => None,
        };
        if let Some(msg) = first {
            pending.absorb(msg);
        } else {
            retry_at = None;
            pending.add_reason("deferred reconciliation".to_string());
        }

        let started = Instant::now();
        let mut closed = false;
        loop {
            let left = settings.max_batch_wait().saturating_sub(started.elapsed());
            let wait = settings.debounce.min(left);
            if wait.is_zero() {
                break;
            }
            tokio::select! {
                () = cancel.cancelled() => return,
                received = tokio::time::timeout(wait, rx.recv()) => match received {
                    Ok(Some(msg)) => pending.absorb(msg),
                    Ok(None) => {
                        closed = true;
                        break;
                    }
                    // Quiet for a whole debounce period.
                    Err(_) => break,
                },
            }
        }
        if overflow.swap(false, Ordering::AcqRel) {
            pending.add_reason("the event queue was full and events were dropped".to_string());
        }
        if !pending.is_empty() {
            let outcome = apply(pending.into_batch()).await;
            if !outcome.new_dirs.is_empty() {
                register(outcome.new_dirs);
            }
            if outcome.reconciled {
                retry_at = None;
            }
            if let Some(after) = outcome.retry_rescan_after {
                retry_at = Some(Instant::now() + after);
            }
        }
        if closed {
            return;
        }
    }
}

/// Sleeps until `deadline`, or forever without one.
async fn sleep_until_opt(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// The native watcher and the directories it holds.
pub(crate) struct NativeWatcher {
    watcher: notify::RecommendedWatcher,
    mode: WatchMode,
    watched: HashSet<PathBuf>,
    stats: Arc<WatchStats>,
    limit_reported: bool,
}

impl NativeWatcher {
    /// Creates the native watcher and registers `root` (recursive mode) or
    /// each of `dirs` (per-directory mode). Events go to `tx`; a full
    /// channel sets `overflow` instead of blocking the notify thread.
    pub(crate) fn start(
        root: &Path,
        mode: WatchMode,
        dirs: Vec<PathBuf>,
        tx: mpsc::Sender<WatchMsg>,
        overflow: Arc<AtomicBool>,
        stats: Arc<WatchStats>,
    ) -> Result<Self, String> {
        let callback_stats = Arc::clone(&stats);
        let handler = move |result: notify::Result<notify::Event>| {
            let Some(msg) = event_to_message(result) else {
                return;
            };
            if tx.try_send(msg).is_err() {
                overflow.store(true, Ordering::Release);
                callback_stats
                    .dropped_events
                    .fetch_add(1, Ordering::Relaxed);
            }
        };
        let watcher = notify::recommended_watcher(handler)
            .map_err(|e| format!("could not create the native watcher: {e}"))?;
        let mut native = Self {
            watcher,
            mode,
            watched: HashSet::new(),
            stats,
            limit_reported: false,
        };
        match mode {
            WatchMode::RecursiveRoot => {
                native
                    .watcher
                    .watch(root, RecursiveMode::Recursive)
                    .map_err(|e| describe_watch_error(root, &e))?;
                native.watched.insert(root.to_path_buf());
            }
            WatchMode::PerDirectory => {
                for dir in dirs {
                    native
                        .watcher
                        .watch(&dir, RecursiveMode::NonRecursive)
                        .map_err(|e| describe_watch_error(&dir, &e))?;
                    native.watched.insert(dir);
                }
            }
        }
        native.publish_count();
        Ok(native)
    }

    /// Registers directories that appeared after start (per-directory mode
    /// only; a recursive watch already covers them).
    pub(crate) fn add_dirs(&mut self, dirs: Vec<PathBuf>) {
        if self.mode != WatchMode::PerDirectory {
            return;
        }
        for dir in dirs {
            if self.watched.contains(&dir) {
                continue;
            }
            if self.watched.len() >= MAX_WATCHED_DIRS {
                if !self.limit_reported {
                    self.limit_reported = true;
                    self.stats.record_error(&format!(
                        "{MAX_WATCHED_DIRS} directories are already watched; changes in new \
                         directories are picked up by the on-demand staleness check only"
                    ));
                }
                break;
            }
            match self.watcher.watch(&dir, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    self.watched.insert(dir);
                }
                // Gone again before it could be registered: nothing to watch.
                Err(e) if matches!(e.kind, notify::ErrorKind::PathNotFound) => {}
                Err(e) => {
                    self.stats.record_error(&describe_watch_error(&dir, &e));
                    if matches!(e.kind, notify::ErrorKind::MaxFilesWatch) {
                        break;
                    }
                }
            }
        }
        self.publish_count();
    }

    fn publish_count(&self) {
        self.stats.watched_dirs.store(
            u64::try_from(self.watched.len()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }
}

/// An actionable description of a failure to register `path`.
fn describe_watch_error(path: &Path, error: &notify::Error) -> String {
    match &error.kind {
        notify::ErrorKind::MaxFilesWatch => format!(
            "the operating system's watch limit was reached while registering {}; raise it \
             (on Linux: sysctl fs.inotify.max_user_watches) or exclude large directories \
             with `exclude` globs",
            path.display()
        ),
        notify::ErrorKind::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => {
            format!(
                "permission denied while watching {}; exclude it with an `exclude` glob or \
                 fix its permissions",
                path.display()
            )
        }
        _ => format!("could not watch {}: {error}", path.display()),
    }
}

/// The running watcher task, owned by the server until shutdown.
#[derive(Debug)]
pub struct FsWatchHandle {
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl FsWatchHandle {
    pub(crate) fn new(cancel: CancellationToken, task: tokio::task::JoinHandle<()>) -> Self {
        Self { cancel, task }
    }

    /// Stops the watcher and waits, bounded, for its task to finish. The
    /// native watches are released when the task drops its watcher.
    pub async fn stop(self) {
        self.cancel.cancel();
        if tokio::time::timeout(SHUTDOWN_TIMEOUT, self.task)
            .await
            .is_err()
        {
            eprintln!(
                "[tokensave] filesystem watch: a batch was still being applied after {}s; \
                 exiting without waiting for it",
                SHUTDOWN_TIMEOUT.as_secs()
            );
        }
    }
}

/// Records the watcher's configuration once it starts.
pub(crate) fn mark_started(stats: &WatchStats, mode: WatchMode, settings: WatchSettings) {
    if let Ok(mut guard) = stats.mode.lock() {
        *guard = Some(mode.as_str());
    }
    stats.debounce_ms.store(
        u64::try_from(settings.debounce.as_millis()).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
}

/// Moves the watcher to `state` (`starting`, `active`, `failed`, `stopped`).
pub(crate) fn set_state(stats: &WatchStats, state: &'static str) {
    stats.set_state(state);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, RemoveKind, RenameMode};

    fn change(path: &str, structural: bool) -> WatchedChange {
        WatchedChange {
            path: PathBuf::from(path),
            structural,
        }
    }

    #[test]
    fn debounce_setting_is_parsed_and_clamped() {
        assert_eq!(
            WatchSettings::from_debounce_setting("2s").debounce,
            Duration::from_secs(2)
        );
        assert_eq!(
            WatchSettings::from_debounce_setting("500ms").debounce,
            Duration::from_millis(500)
        );
        assert_eq!(
            WatchSettings::from_debounce_setting("1ms").debounce,
            MIN_DEBOUNCE
        );
        assert_eq!(
            WatchSettings::from_debounce_setting("10m").debounce,
            MAX_DEBOUNCE
        );
        assert_eq!(
            WatchSettings::from_debounce_setting("soon").debounce,
            DEFAULT_DEBOUNCE
        );
    }

    #[test]
    fn events_are_classified() {
        assert_eq!(classify_event(&EventKind::Access(AccessKind::Any)), None);
        assert_eq!(
            classify_event(&EventKind::Create(CreateKind::File)),
            Some(true)
        );
        assert_eq!(
            classify_event(&EventKind::Remove(RemoveKind::Any)),
            Some(true)
        );
        assert_eq!(
            classify_event(&EventKind::Modify(ModifyKind::Name(RenameMode::Both))),
            Some(true)
        );
        assert_eq!(
            classify_event(&EventKind::Modify(ModifyKind::Data(DataChange::Content))),
            Some(false)
        );
    }

    #[test]
    fn noise_paths_are_dropped_before_queueing() {
        assert!(is_noise_path(Path::new("/p/.git/index")));
        assert!(is_noise_path(Path::new("/p/.tokensave/tokensave.db-wal")));
        assert!(is_noise_path(Path::new("/p/src/.main.rs.swp")));
        assert!(!is_noise_path(Path::new("/p/src/main.rs")));
    }

    #[test]
    fn rescan_flag_and_errors_become_rescans() {
        let err = notify::Error::generic("boom");
        assert!(matches!(
            event_to_message(Err(err)),
            Some(WatchMsg::Rescan(_))
        ));
        let event = notify::Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert!(matches!(
            event_to_message(Ok(event)),
            Some(WatchMsg::Rescan(_))
        ));
        let access = notify::Event::new(EventKind::Access(AccessKind::Any))
            .add_path(PathBuf::from("/p/src/main.rs"));
        assert_eq!(event_to_message(Ok(access)), None);
    }

    #[test]
    fn pending_set_is_bounded_and_overflows_into_rescan() {
        let mut pending = Pending::default();
        let many: Vec<WatchedChange> = (0..=MAX_PENDING_PATHS)
            .map(|i| change(&format!("/p/f{i}.rs"), false))
            .collect();
        pending.absorb(WatchMsg::Changes(many));
        assert_eq!(pending.paths.len(), MAX_PENDING_PATHS);
        assert!(pending.rescan);
        // Once overflowed, further paths are not collected.
        pending.absorb(WatchMsg::Changes(vec![change("/p/extra.rs", false)]));
        assert_eq!(pending.paths.len(), MAX_PENDING_PATHS);
    }

    #[test]
    fn repeated_paths_merge_and_keep_structural() {
        let mut pending = Pending::default();
        pending.absorb(WatchMsg::Changes(vec![change("/p/a.rs", false)]));
        pending.absorb(WatchMsg::Changes(vec![change("/p/a.rs", true)]));
        pending.absorb(WatchMsg::Changes(vec![change("/p/a.rs", false)]));
        let batch = pending.into_batch();
        assert_eq!(batch.changes, vec![change("/p/a.rs", true)]);
        assert!(!batch.rescan);
    }

    #[tokio::test(start_paused = true)]
    async fn bursts_are_debounced_into_one_batch() {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let overflow = Arc::new(AtomicBool::new(false));
        let cancel = CancellationToken::new();
        let batches = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&batches);
        let settings = WatchSettings {
            debounce: Duration::from_millis(200),
        };
        let task = tokio::spawn(run_watch_loop(
            rx,
            Arc::clone(&overflow),
            settings,
            cancel.clone(),
            move |batch| {
                sink.lock().unwrap().push(batch);
                async { WatchOutcome::default() }
            },
            |_| {},
        ));
        for i in 0..20 {
            tx.send(WatchMsg::Changes(vec![change("/p/a.rs", i % 2 == 0)]))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        {
            let got = batches.lock().unwrap();
            assert_eq!(got.len(), 1, "one burst, one batch: {got:?}");
            assert_eq!(got[0].changes, vec![change("/p/a.rs", true)]);
        }
        cancel.cancel();
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn overflow_flag_turns_the_batch_into_a_rescan() {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let overflow = Arc::new(AtomicBool::new(false));
        let cancel = CancellationToken::new();
        let batches = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&batches);
        let task = tokio::spawn(run_watch_loop(
            rx,
            Arc::clone(&overflow),
            WatchSettings {
                debounce: Duration::from_millis(100),
            },
            cancel.clone(),
            move |batch| {
                sink.lock().unwrap().push(batch);
                async { WatchOutcome::default() }
            },
            |_| {},
        ));
        overflow.store(true, Ordering::Release);
        tx.send(WatchMsg::Changes(vec![change("/p/a.rs", false)]))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        {
            let got = batches.lock().unwrap();
            assert_eq!(got.len(), 1);
            assert!(got[0].rescan);
        }
        cancel.cancel();
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn deferred_reconciliation_fires_without_new_events() {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let cancel = CancellationToken::new();
        let batches = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&batches);
        let task = tokio::spawn(run_watch_loop(
            rx,
            Arc::new(AtomicBool::new(false)),
            WatchSettings {
                debounce: Duration::from_millis(100),
            },
            cancel.clone(),
            move |batch: WatchBatch| {
                let first = {
                    let mut got = sink.lock().unwrap();
                    got.push(batch);
                    got.len() == 1
                };
                async move {
                    WatchOutcome {
                        retry_rescan_after: first.then_some(Duration::from_secs(10)),
                        ..WatchOutcome::default()
                    }
                }
            },
            |_| {},
        ));
        tx.send(WatchMsg::Rescan("test".to_string())).await.unwrap();
        tokio::time::sleep(Duration::from_secs(11)).await;
        {
            let got = batches.lock().unwrap();
            assert_eq!(got.len(), 2, "{got:?}");
            assert!(got[1].rescan);
        }
        cancel.cancel();
        task.await.unwrap();
        drop(tx);
    }

    #[tokio::test]
    async fn loop_ends_when_cancelled_and_when_senders_close() {
        let (tx, rx) = mpsc::channel::<WatchMsg>(1);
        let cancel = CancellationToken::new();
        let task = tokio::spawn(run_watch_loop(
            rx,
            Arc::new(AtomicBool::new(false)),
            WatchSettings {
                debounce: MIN_DEBOUNCE,
            },
            cancel.clone(),
            |_| async { WatchOutcome::default() },
            |_| {},
        ));
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        drop(tx);

        let (tx, rx) = mpsc::channel::<WatchMsg>(1);
        let task = tokio::spawn(run_watch_loop(
            rx,
            Arc::new(AtomicBool::new(false)),
            WatchSettings {
                debounce: MIN_DEBOUNCE,
            },
            CancellationToken::new(),
            |_| async { WatchOutcome::default() },
            |_| {},
        ));
        drop(tx);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[test]
    fn stats_report_off_until_started() {
        let stats = WatchStats::default();
        assert_eq!(stats.to_json(), json!({ "state": "off" }));
        set_state(&stats, "active");
        mark_started(
            &stats,
            WatchMode::PerDirectory,
            WatchSettings {
                debounce: DEFAULT_DEBOUNCE,
            },
        );
        stats.record_error("x");
        let value = stats.to_json();
        assert_eq!(value["state"], "active");
        assert_eq!(value["mode"], "per-directory");
        assert_eq!(value["errors"], 1);
        assert_eq!(value["last_error"], "x");
    }
}
