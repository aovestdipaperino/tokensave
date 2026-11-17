//! Reuse the production recorder, operation registry and histogram layout.
//! Each sequential probe owns four bounded engine recorders, released at its end.
//! SQL and row values never enter metric labels. Off by default.
#![allow(dead_code)]

#[path = "../../../src/profile/histogram.rs"]
pub mod histogram;
#[path = "../../../src/profile/ops.rs"]
pub mod ops;
#[path = "../../../src/profile/recorder.rs"]
pub mod recorder;

use crate::engine::EngineKind;
use ops::DbOp;
use recorder::{Recorder, Sample};
use std::sync::Arc;
use std::time::Instant;

tokio::task_local! { static METRICS: Arc<[Recorder; 4]>; }

fn index(kind: EngineKind) -> usize {
    match kind {
        EngineKind::Libsql => 0,
        EngineKind::Turso => 1,
        EngineKind::TursoExperimental => 2,
        EngineKind::TursoMultiprocessWal => 3,
    }
}

pub async fn scoped<F: std::future::Future>(
    enabled: bool,
    future: F,
) -> (F::Output, serde_json::Value) {
    if !enabled {
        return (future.await, serde_json::Value::Null);
    }
    let metrics = Arc::new(std::array::from_fn(|_| Recorder::new()));
    let result = METRICS.scope(metrics.clone(), future).await;
    let mut data = serde_json::Map::new();
    for kind in EngineKind::ALL {
        let snapshot = metrics[index(kind)].drain();
        if !snapshot.is_empty() {
            data.insert(
                kind.label().into(),
                serde_json::to_value(snapshot).unwrap_or_default(),
            );
        }
    }
    (result, serde_json::Value::Object(data))
}

pub struct Span {
    data: Option<(Arc<[Recorder; 4]>, usize, DbOp, Instant)>,
}
impl Span {
    pub fn start(kind: EngineKind, op: DbOp) -> Self {
        Self {
            data: METRICS
                .try_with(|metrics| (metrics.clone(), index(kind), op, Instant::now()))
                .ok(),
        }
    }
    pub fn finish(mut self, returned: u64) {
        if let Some((metrics, index, op, start)) = self.data.take() {
            let us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
            metrics[index].record(
                op.into(),
                Sample {
                    inclusive_us: us,
                    exclusive_us: us,
                    rows_returned: returned,
                    rows_affected: 0,
                },
            );
        }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some((metrics, index, op, _)) = self.data.take() {
            metrics[index].record_error(op.into());
        }
    }
}
