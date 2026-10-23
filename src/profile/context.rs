//! Spans and per-invocation attribution.
//!
//! A [`Span`] measures one operation. While it is open it is the innermost
//! frame of the current context, so time spent in nested spans is subtracted
//! from its exclusive time, and database spans add to the invocation's
//! database time.
//!
//! The context is a tokio task-local inside [`invocation`] (an MCP call or a
//! sync run, possibly crossing `.await` points and threads), and a
//! thread-local inside [`invocation_sync`] (synchronous code). Outside both,
//! spans have no context: they are recorded with exclusive time equal to
//! inclusive and attributed to no invocation. That keeps a span held across
//! an `.await` outside an invocation, which may resume on another thread,
//! from corrupting a thread's frame stack. Frames live in a fixed array, so
//! opening a span never allocates; spans nested deeper than [`MAX_DEPTH`]
//! are still recorded, with exclusive time equal to inclusive.
//!
//! [`invocation`]: super::invocation
//! [`invocation_sync`]: super::invocation_sync

use std::cell::RefCell;
use std::future::Future;
use std::time::Instant;

use super::ops::{Category, Op};
use super::recorder::{Recorder, Sample};

/// Deepest nesting with exclusive-time accounting.
pub const MAX_DEPTH: usize = 16;

#[derive(Clone, Copy)]
struct Frame {
    child_us: u64,
    db: bool,
}

const EMPTY_FRAME: Frame = Frame {
    child_us: 0,
    db: false,
};

/// Database time and operation counts for one invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvocationSummary {
    /// Time in outermost database spans; nested database spans are not
    /// counted twice.
    pub db_us: u64,
    /// Number of database spans, nested or not.
    pub db_ops: u64,
    /// Executions per database op (indexed by [`Op::index`], saturating),
    /// for N+1 detection. A fixed array, so an invocation never allocates.
    pub op_counts: [u16; Op::COUNT],
}

impl InvocationSummary {
    const EMPTY: Self = Self {
        db_us: 0,
        db_ops: 0,
        op_counts: [0; Op::COUNT],
    };

    /// Database ops executed at least `min` times, with their counts.
    pub fn repeated_ops(&self, min: u16) -> impl Iterator<Item = (Op, u16)> + '_ {
        self.op_counts
            .iter()
            .enumerate()
            .filter(move |(_, n)| **n >= min)
            .filter_map(|(index, n)| Op::from_index(index).map(|op| (op, *n)))
    }
}

impl Default for InvocationSummary {
    fn default() -> Self {
        Self::EMPTY
    }
}

struct Context {
    frames: [Frame; MAX_DEPTH],
    depth: usize,
    summary: InvocationSummary,
    /// Whether an invocation owns this context. The idle thread-local
    /// context is inactive and ignored by spans.
    active: bool,
}

impl Context {
    const fn new(active: bool) -> Self {
        Self {
            frames: [EMPTY_FRAME; MAX_DEPTH],
            depth: 0,
            summary: InvocationSummary::EMPTY,
            active,
        }
    }

    /// Pushes a frame and returns its slot, or `None` past `MAX_DEPTH`.
    fn push(&mut self, db: bool) -> Option<usize> {
        let slot = self.depth;
        self.depth += 1;
        let frame = self.frames.get_mut(slot)?;
        *frame = Frame { child_us: 0, db };
        Some(slot)
    }

    /// Pops the frame opened at `slot` and returns its child time. A guard
    /// dropped out of order unwinds any frames opened after it.
    fn pop(&mut self, slot: Option<usize>, op: Op, elapsed_us: u64) -> u64 {
        let depth_after = match slot {
            Some(slot) => slot,
            None => self.depth.saturating_sub(1),
        };
        let child_us = slot
            .and_then(|s| self.frames.get(s))
            .map_or(0, |f| f.child_us);
        self.depth = depth_after.min(self.depth);
        let parent = self
            .depth
            .checked_sub(1)
            .and_then(|p| self.frames.get_mut(p));
        let parent_is_db = parent.as_ref().is_some_and(|p| p.db);
        if let Some(parent) = parent {
            parent.child_us = parent.child_us.saturating_add(elapsed_us);
        }
        if op.category() == Category::Db {
            self.summary.db_ops += 1;
            if let Some(n) = self.summary.op_counts.get_mut(op.index()) {
                *n = n.saturating_add(1);
            }
            if !parent_is_db {
                self.summary.db_us = self.summary.db_us.saturating_add(elapsed_us);
            }
        }
        child_us
    }
}

tokio::task_local! {
    static TASK_CONTEXT: RefCell<Context>;
}

thread_local! {
    static THREAD_CONTEXT: RefCell<Context> = const { RefCell::new(Context::new(false)) };
}

/// Runs `f` on the active invocation's context. Returns `None` outside an
/// invocation, during thread teardown, or if the context is already
/// borrowed.
fn with_context<R>(f: impl FnOnce(&mut Context) -> R) -> Option<R> {
    with_current(|c| c.active.then(|| f(c))).flatten()
}

/// Runs `f` on the task-local context if one is in scope, else on this
/// thread's context, active or not.
fn with_current<R>(f: impl FnOnce(&mut Context) -> R) -> Option<R> {
    if TASK_CONTEXT.try_with(|_| ()).is_ok() {
        TASK_CONTEXT
            .try_with(|c| c.try_borrow_mut().ok().map(|mut c| f(&mut c)))
            .ok()
            .flatten()
    } else {
        THREAD_CONTEXT
            .try_with(|c| c.try_borrow_mut().ok().map(|mut c| f(&mut c)))
            .ok()
            .flatten()
    }
}

/// Runs `fut` as one invocation with its own context, returning its output
/// and the invocation's database attribution. Spans opened inside are
/// attributed to it even across `.await` points and worker threads.
pub(crate) async fn scoped<F: Future>(fut: F) -> (F::Output, InvocationSummary) {
    TASK_CONTEXT
        .scope(RefCell::new(Context::new(true)), async move {
            let out = fut.await;
            let summary = TASK_CONTEXT
                .try_with(|c| c.try_borrow().map(|c| c.summary).unwrap_or_default())
                .unwrap_or_default();
            (out, summary)
        })
        .await
}

/// Synchronous counterpart of [`scoped`]: gives `f` a fresh active context
/// (the task-local one when called inside [`scoped`], else this thread's)
/// and restores the previous one afterwards.
pub(crate) fn scoped_sync<R>(f: impl FnOnce() -> R) -> (R, InvocationSummary) {
    let saved = with_current(|c| std::mem::replace(c, Context::new(true)));
    let out = f();
    let summary = with_current(|c| {
        let summary = c.summary;
        if let Some(saved) = saved {
            *c = saved;
        }
        summary
    })
    .unwrap_or_default();
    (out, summary)
}

/// Guard measuring one operation; see the module docs.
///
/// Call [`Span::finish`] on success. A span dropped without it (an early
/// return through `?`, a panic, a cancelled future) or ended with
/// [`Span::fail`] counts as an error and stays out of the latency histogram;
/// its time is still subtracted from the enclosing span.
#[must_use = "a span measures until it is finished or dropped"]
pub struct Span {
    active: Option<Active>,
}

struct Active {
    recorder: &'static Recorder,
    op: Op,
    start: Instant,
    /// `None` when no context was active at start; otherwise the frame slot
    /// (`Some(None)` past `MAX_DEPTH`).
    slot: Option<Option<usize>>,
    rows_returned: u64,
    rows_affected: u64,
}

impl Span {
    /// A span that measures nothing (profiling off).
    pub const fn inert() -> Self {
        Self { active: None }
    }

    pub(crate) fn start(recorder: &'static Recorder, op: Op) -> Self {
        let slot = with_context(|c| c.push(op.category() == Category::Db));
        Self {
            active: Some(Active {
                recorder,
                op,
                start: Instant::now(),
                slot,
                rows_returned: 0,
                rows_affected: 0,
            }),
        }
    }

    /// Whether this span is measuring.
    pub const fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Adds rows returned (read) and affected (written).
    pub fn add_rows(&mut self, returned: u64, affected: u64) {
        if let Some(a) = &mut self.active {
            a.rows_returned = a.rows_returned.saturating_add(returned);
            a.rows_affected = a.rows_affected.saturating_add(affected);
        }
    }

    /// Ends the span successfully.
    pub fn finish(mut self) {
        let _ = self.complete(true);
    }

    /// Ends the span as failed.
    pub fn fail(mut self) {
        let _ = self.complete(false);
    }

    /// Ends the span as succeeded or failed and returns its inclusive time,
    /// or `None` for an inert span.
    pub fn end(mut self, ok: bool) -> Option<u64> {
        self.complete(ok)
    }

    fn complete(&mut self, ok: bool) -> Option<u64> {
        let a = self.active.take()?;
        let inclusive_us = u64::try_from(a.start.elapsed().as_micros()).unwrap_or(u64::MAX);
        let child_us = a
            .slot
            .and_then(|slot| with_context(|c| c.pop(slot, a.op, inclusive_us)))
            .unwrap_or(0);
        if ok {
            a.recorder.record(
                a.op,
                Sample {
                    inclusive_us,
                    exclusive_us: inclusive_us.saturating_sub(child_us),
                    rows_returned: a.rows_returned,
                    rows_affected: a.rows_affected,
                },
            );
        } else {
            a.recorder.record_error(a.op);
        }
        Some(inclusive_us)
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        let _ = self.complete(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ops::{DbOp, McpPhase};
    use std::time::Duration;

    fn recorder() -> &'static Recorder {
        Box::leak(Box::new(Recorder::new()))
    }

    fn spin(d: Duration) {
        let start = Instant::now();
        while start.elapsed() < d {
            std::hint::spin_loop();
        }
    }

    #[test]
    fn exclusive_time_excludes_children() {
        let r = recorder();
        let (_, summary) = scoped_sync(|| {
            let outer = Span::start(r, Op::Mcp(McpPhase::Total));
            spin(Duration::from_millis(2));
            let inner = Span::start(r, Op::Db(DbOp::Transaction));
            spin(Duration::from_millis(5));
            inner.finish();
            outer.finish();
        });
        let snap = r.drain();
        let outer = &snap["mcp.total"];
        let inner = &snap["db.transaction"];
        assert_eq!(inner.exclusive_sum_us, inner.sum_us);
        assert!(inner.sum_us >= 5_000);
        assert!(outer.sum_us >= inner.sum_us + 2_000);
        assert_eq!(outer.exclusive_sum_us, outer.sum_us - inner.sum_us);
        assert_eq!(summary.db_ops, 1);
        assert_eq!(summary.db_us, inner.sum_us);
    }

    #[test]
    fn nested_db_spans_are_not_double_counted() {
        let r = recorder();
        let (_, summary) = scoped_sync(|| {
            let tx = Span::start(r, Op::Db(DbOp::Transaction));
            let lock = Span::start(r, Op::Db(DbOp::WriteLockWait));
            spin(Duration::from_millis(1));
            lock.finish();
            tx.finish();
        });
        let snap = r.drain();
        assert_eq!(summary.db_ops, 2);
        assert_eq!(summary.db_us, snap["db.transaction"].sum_us);
    }

    #[test]
    fn dropped_span_is_an_error_but_still_a_child() {
        let r = recorder();
        scoped_sync(|| {
            let outer = Span::start(r, Op::Mcp(McpPhase::Total));
            {
                let _failed = Span::start(r, Op::Db(DbOp::Open));
                spin(Duration::from_millis(2));
            }
            outer.finish();
        });
        let snap = r.drain();
        assert_eq!(snap["db.open"].errors, 1);
        assert_eq!(snap["db.open"].count, 0);
        let total = &snap["mcp.total"];
        assert!(total.sum_us - total.exclusive_sum_us >= 2_000);
    }

    #[test]
    fn deep_nesting_past_the_frame_limit_is_still_recorded() {
        let r = recorder();
        let (depth, _) = scoped_sync(|| {
            let spans: Vec<_> = (0..MAX_DEPTH + 4)
                .map(|_| Span::start(r, Op::Mcp(McpPhase::Ranking)))
                .collect();
            for span in spans.into_iter().rev() {
                span.finish();
            }
            with_context(|c| c.depth)
        });
        assert_eq!(r.drain()["mcp.ranking"].count, (MAX_DEPTH + 4) as u64);
        assert_eq!(depth, Some(0));
    }

    #[test]
    fn out_of_order_drop_unwinds_the_stack() {
        let r = recorder();
        let (depth, _) = scoped_sync(|| {
            let a = Span::start(r, Op::Mcp(McpPhase::Total));
            let b = Span::start(r, Op::Mcp(McpPhase::Ranking));
            a.finish();
            b.finish();
            with_context(|c| c.depth)
        });
        assert_eq!(depth, Some(0));
        assert_eq!(r.drain().len(), 2);
    }

    #[test]
    fn spans_outside_an_invocation_leave_the_thread_context_alone() {
        let r = recorder();
        let outer = Span::start(r, Op::Mcp(McpPhase::Total));
        let inner = Span::start(r, Op::Db(DbOp::Open));
        inner.finish();
        outer.finish();
        assert_eq!(with_current(|c| (c.active, c.depth)), Some((false, 0)));
        let snap = r.drain();
        let total = &snap["mcp.total"];
        assert_eq!(
            total.exclusive_sum_us, total.sum_us,
            "no parent attribution"
        );
    }

    #[test]
    fn inert_span_records_nothing() {
        let mut s = Span::inert();
        assert!(!s.is_active());
        s.add_rows(1, 1);
        s.finish();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn task_invocations_are_isolated_across_awaits() {
        let r = recorder();
        let task = |n: u64| async move {
            scoped(async move {
                for _ in 0..n {
                    let s = Span::start(r, Op::Db(DbOp::Open));
                    tokio::task::yield_now().await;
                    s.finish();
                }
            })
            .await
            .1
        };
        let (a, b) = tokio::join!(tokio::spawn(task(3)), tokio::spawn(task(7)));
        assert_eq!(a.unwrap().db_ops, 3);
        assert_eq!(b.unwrap().db_ops, 7);
        assert_eq!(r.drain()["db.open"].count, 10);
    }
}
