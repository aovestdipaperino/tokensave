//! Subprocess-isolated extraction.
//!
//! Tree-sitter grammars compiled from C/C++ can `abort()` on internal
//! assertions, segfault, or otherwise terminate the process by paths that
//! `catch_unwind` cannot intercept. To keep `tokensave sync` resilient,
//! extraction is delegated to short-lived worker subprocesses; if a worker
//! dies, only the in-flight file is lost and the pool respawns the worker.
//!
//! ## Trust boundary
//!
//! The worker entry point is a hidden subcommand (`tokensave extract-worker`)
//! that authenticates via two facts the parent controls:
//!
//! 1. A 32-byte token, freshly generated per `WorkerPool`, passed via the
//!    `TOKENSAVE_WORKER_TOKEN` env var (hex-encoded). The worker scrubs the
//!    var immediately after reading.
//! 2. The first 32 bytes received on stdin must equal the same token.
//!
//! A user invoking `tokensave extract-worker` directly hits the missing-env
//! check and exits non-zero. A user who guesses or extracts the env value
//! still cannot reproduce the stdin handshake without being inside the
//! parent's address space — at which point the trust boundary is moot.

use std::collections::VecDeque;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::extraction::LanguageRegistry;
use crate::sync;
use crate::types::ExtractionResult;

const TOKEN_LEN: usize = 32;
const TOKEN_ENV_VAR: &str = "TOKENSAVE_WORKER_TOKEN";

/// Hidden subcommand name. Kept here (not in main.rs) so the constant is
/// shared between the spawn-side and the dispatch-side.
pub const WORKER_SUBCOMMAND: &str = "extract-worker";

#[derive(Serialize, Deserialize)]
struct ExtractRequest {
    project_root: PathBuf,
    file_path: String,
}

#[derive(Serialize, Deserialize)]
struct ExtractResponse {
    file_path: String,
    /// `Some` on success. `None` means the file was unreadable or had no
    /// matching extractor — both legitimate outcomes that aren't worth a
    /// crash. Extractor panics kill the worker entirely; the pool sees the
    /// pipe close and respawns.
    data: Option<ExtractData>,
}

#[derive(Serialize, Deserialize)]
struct ExtractData {
    result: ExtractionResult,
    content_hash: String,
    size: u64,
    mtime: i64,
}

fn generate_token() -> io::Result<[u8; TOKEN_LEN]> {
    let mut buf = [0u8; TOKEN_LEN];
    getrandom::getrandom(&mut buf)
        .map_err(|e| io::Error::other(format!("getrandom failed: {e}")))?;
    Ok(buf)
}

// =============================================================================
// Worker side — runs inside the spawned child
// =============================================================================

/// Worker entry point. Never returns; calls `process::exit`.
pub fn run_worker() -> ! {
    let code = match worker_main() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[tokensave-worker] {e}");
            1
        }
    };
    std::process::exit(code);
}

fn worker_main() -> io::Result<()> {
    let token_hex = std::env::var(TOKEN_ENV_VAR).map_err(|_| {
        io::Error::other("worker token not set; cannot run extract-worker directly")
    })?;
    // Scrub immediately so a child of a child cannot inherit it.
    std::env::remove_var(TOKEN_ENV_VAR);
    let expected =
        hex::decode(token_hex.trim()).map_err(|_| io::Error::other("worker token malformed"))?;
    if expected.len() != TOKEN_LEN {
        return Err(io::Error::other("worker token wrong length"));
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    let mut received = [0u8; TOKEN_LEN];
    reader.read_exact(&mut received)?;
    // Constant-time-ish comparison; the token isn't a long-term secret but
    // there's no reason to leak timing.
    if !slices_eq(&received, &expected) {
        return Err(io::Error::other("worker token mismatch"));
    }

    let registry = LanguageRegistry::new();
    loop {
        let req: ExtractRequest = match read_message(&mut reader) {
            Ok(req) => req,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        };
        let resp = process_request(&registry, &req);
        write_message(&mut writer, &resp)?;
        writer.flush()?;
    }
}

fn process_request(registry: &LanguageRegistry, req: &ExtractRequest) -> ExtractResponse {
    let abs_path = req.project_root.join(&req.file_path);
    let Ok(source) = sync::read_source_file(&abs_path) else {
        return ExtractResponse {
            file_path: req.file_path.clone(),
            data: None,
        };
    };
    let Some(extractor) = registry.extractor_for_file(&req.file_path) else {
        return ExtractResponse {
            file_path: req.file_path.clone(),
            data: None,
        };
    };

    let mut result = extractor.extract(&req.file_path, &source);
    result.sanitize();
    let content_hash = sync::content_hash(&source);
    let size = source.len() as u64;
    let mtime =
        sync::file_stat(&abs_path).map_or_else(crate::tokensave::current_timestamp, |(m, _)| m);

    ExtractResponse {
        file_path: req.file_path.clone(),
        data: Some(ExtractData {
            result,
            content_hash,
            size,
            mtime,
        }),
    }
}

// =============================================================================
// Pool side — runs inside the parent
// =============================================================================

/// One result tuple. Matches the shape the existing extraction sites in
/// `tokensave.rs` expect from their rayon closures.
pub type ExtractTuple = (String, ExtractionResult, String, u64, i64);

pub struct WorkerPool {
    workers: Vec<WorkerHandle>,
    self_path: PathBuf,
    project_root: PathBuf,
    token: [u8; TOKEN_LEN],
}

struct WorkerHandle {
    /// `None` once the handle is being dropped: closing the pipe is what
    /// signals the worker to exit, and we have to do it before `wait()`.
    stdin: Option<BufWriter<ChildStdin>>,
    stdout: BufReader<ChildStdout>,
    child: Child,
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        // Close the worker's stdin first; otherwise it sits in `read_exact`
        // forever and `wait()` deadlocks waiting for it to exit.
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

impl WorkerPool {
    /// Spawn `num_workers` worker processes. Each gets the same token; the
    /// token is generated once per pool.
    pub fn new(num_workers: usize, project_root: PathBuf) -> io::Result<Self> {
        let self_path = std::env::current_exe()?;
        let token = generate_token()?;
        let mut workers = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            workers.push(spawn_worker(&self_path, &token)?);
        }
        Ok(Self {
            workers,
            self_path,
            project_root,
            token,
        })
    }

    /// Process every entry in `files`, calling `on_progress(n, total, path)`
    /// once per file. Returns one tuple per successfully-processed file;
    /// files whose worker crashed or that had no extractor / read error are
    /// silently skipped (logged to stderr).
    pub fn extract_files<F>(self, files: Vec<String>, on_progress: F) -> Vec<ExtractTuple>
    where
        F: Fn(usize, usize, &str) + Send + Sync + 'static,
    {
        let total = files.len();
        let queue: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(files.into_iter().collect()));
        let results: Arc<Mutex<Vec<ExtractTuple>>> =
            Arc::new(Mutex::new(Vec::with_capacity(total)));
        let progress_count = Arc::new(AtomicUsize::new(0));
        let on_progress = Arc::new(on_progress);

        let handles: Vec<_> = self
            .workers
            .into_iter()
            .map(|worker| {
                let queue = queue.clone();
                let results = results.clone();
                let progress_count = progress_count.clone();
                let on_progress = on_progress.clone();
                let project_root = self.project_root.clone();
                let self_path = self.self_path.clone();
                let token = self.token;

                std::thread::spawn(move || {
                    worker_thread(
                        worker,
                        queue,
                        results,
                        progress_count,
                        on_progress,
                        project_root,
                        self_path,
                        token,
                        total,
                    );
                })
            })
            .collect();

        for h in handles {
            let _ = h.join();
        }

        // All worker threads have joined, so we hold the only Arc strong
        // reference. `into_inner` returns `Some` in that case; if it ever
        // returns `None` (concurrent leak), prefer an empty result over a
        // panic — the sync continues and the user just sees zero changes.
        Arc::into_inner(results)
            .and_then(|m| m.into_inner().ok())
            .unwrap_or_default()
    }
}

// `worker_thread` is the body of a `thread::spawn` closure that takes
// owned Arc clones / PathBufs by value to keep the strong refcount /
// path data alive for the lifetime of the thread. Clippy's
// `needless_pass_by_value` doesn't model that — it only sees that
// nothing is moved out inside the function — so we silence it here.
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
fn worker_thread<F>(
    mut worker: WorkerHandle,
    queue: Arc<Mutex<VecDeque<String>>>,
    results: Arc<Mutex<Vec<ExtractTuple>>>,
    progress_count: Arc<AtomicUsize>,
    on_progress: Arc<F>,
    project_root: PathBuf,
    self_path: PathBuf,
    token: [u8; TOKEN_LEN],
    total: usize,
) where
    F: Fn(usize, usize, &str) + Send + Sync,
{
    loop {
        let next = queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front();
        let Some(file_path) = next else {
            break;
        };

        let req = ExtractRequest {
            project_root: project_root.clone(),
            file_path: file_path.clone(),
        };

        let outcome = round_trip(&mut worker, &req);
        let n = progress_count.fetch_add(1, Ordering::Relaxed) + 1;
        on_progress(n, total, &file_path);

        match outcome {
            Ok(resp) => {
                if let Some(data) = resp.data {
                    results
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push((
                            resp.file_path,
                            data.result,
                            data.content_hash,
                            data.size,
                            data.mtime,
                        ));
                }
            }
            Err(e) => {
                eprintln!("[tokensave] extraction worker crashed on {file_path}: {e}, respawning");
                // Old `worker` is dropped here, reaping the dead child.
                match spawn_worker(&self_path, &token) {
                    Ok(new_worker) => worker = new_worker,
                    Err(e) => {
                        eprintln!(
                            "[tokensave] failed to respawn worker after crash: {e}; \
                             this thread is giving up, remaining workers continue"
                        );
                        return;
                    }
                }
            }
        }
    }
}

fn round_trip(worker: &mut WorkerHandle, req: &ExtractRequest) -> io::Result<ExtractResponse> {
    let stdin = worker
        .stdin
        .as_mut()
        .ok_or_else(|| io::Error::other("worker stdin already closed"))?;
    write_message(stdin, req)?;
    stdin.flush()?;
    read_message(&mut worker.stdout)
}

fn spawn_worker(self_path: &Path, token: &[u8; TOKEN_LEN]) -> io::Result<WorkerHandle> {
    let token_hex = hex::encode(token);
    let mut child = Command::new(self_path)
        .arg(WORKER_SUBCOMMAND)
        .env(TOKEN_ENV_VAR, token_hex)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("stdin unexpectedly None despite Stdio::piped"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("stdout unexpectedly None despite Stdio::piped"))?;
    let mut stdin = BufWriter::new(stdin);
    let stdout = BufReader::new(stdout);

    stdin.write_all(token)?;
    stdin.flush()?;

    Ok(WorkerHandle {
        stdin: Some(stdin),
        stdout,
        child,
    })
}

// =============================================================================
// Wire format: 4-byte LE length prefix + bincode payload
// =============================================================================

fn read_message<R: Read, T: for<'de> Deserialize<'de>>(reader: &mut R) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    bincode::deserialize(&buf).map_err(io::Error::other)
}

fn write_message<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> io::Result<()> {
    let bytes = bincode::serialize(msg).map_err(io::Error::other)?;
    let len =
        u32::try_from(bytes.len()).map_err(|_| io::Error::other("ipc message exceeds 4 GiB"))?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&bytes)?;
    Ok(())
}

fn slices_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn message_round_trips() {
        let req = ExtractRequest {
            project_root: PathBuf::from("/tmp/x"),
            file_path: "src/main.rs".into(),
        };
        let mut buf = Vec::new();
        write_message(&mut buf, &req).unwrap();
        let mut cursor = std::io::Cursor::new(buf);
        let decoded: ExtractRequest = read_message(&mut cursor).unwrap();
        assert_eq!(decoded.file_path, req.file_path);
        assert_eq!(decoded.project_root, req.project_root);
    }

    #[test]
    fn slices_eq_matches() {
        assert!(slices_eq(b"abc", b"abc"));
        assert!(!slices_eq(b"abc", b"abd"));
        assert!(!slices_eq(b"abc", b"ab"));
    }
}
