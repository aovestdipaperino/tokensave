//! The sanitized export: the only shape in which profile data leaves the
//! profile directory (attached to an issue, or later shared with consent).
//!
//! It contains only the tokensave version, OS family and architecture,
//! logical CPU count, engine names, static series names and their
//! aggregates. Series whose names are not a known static operation or a
//! registered tool are dropped, so a tampered or foreign file cannot smuggle
//! arbitrary strings into it. No paths, project names, symbols, SQL,
//! parameters, hostnames or identifiers are included.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::ops::Op;
use super::recorder::Snapshot;

/// Format identifier of [`Export`].
pub const EXPORT_SCHEMA: &str = "tokensave-profile-export/1";

/// Sanitized, versioned profile aggregates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Export {
    pub schema: String,
    pub tokensave_version: String,
    /// `linux`, `macos`, `windows`, ...
    pub os: String,
    pub arch: String,
    pub cpu_count: usize,
    /// Hours covered, counted back from the export.
    pub window_hours: u64,
    /// Engine name to series.
    pub engines: BTreeMap<String, Snapshot>,
}

/// Tool names the export accepts in `tool.<name>` series: the static tool
/// registry plus the catch-all.
pub fn known_tools() -> BTreeSet<String> {
    let mut tools: BTreeSet<String> = crate::mcp::tools::get_tool_definitions()
        .into_iter()
        .map(|definition| definition.name)
        .collect();
    tools.insert(super::tools::OTHER.to_string());
    tools
}

fn is_known_series(name: &str, tools: &BTreeSet<String>) -> bool {
    match name.strip_prefix("tool.") {
        Some(tool) => tools.contains(tool),
        None => Op::from_name(name).is_some(),
    }
}

/// Builds the sanitized export from merged per-engine data.
pub fn sanitized(
    merged: &BTreeMap<String, Snapshot>,
    window_hours: u64,
    tools: &BTreeSet<String>,
) -> Export {
    let engines = merged
        .iter()
        .filter(|(engine, _)| engine.as_str() == super::storage::ENGINE)
        .map(|(engine, snapshot)| {
            let series = snapshot
                .iter()
                .filter(|(name, _)| is_known_series(name, tools))
                .map(|(name, s)| {
                    let mut s = s.clone();
                    s.repeated.retain(|op, _| Op::from_name(op).is_some());
                    (name.clone(), s)
                })
                .collect();
            (engine.clone(), series)
        })
        .collect();
    Export {
        schema: EXPORT_SCHEMA.to_string(),
        tokensave_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_count: std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
        window_hours,
        engines,
    }
}

/// The export as pretty JSON. This exact text is what `profile export`
/// prints, and what any future upload sends.
pub fn to_json(export: &Export) -> String {
    serde_json::to_string_pretty(export).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profile::recorder::SeriesSnapshot;

    #[test]
    fn export_contains_no_foreign_strings() {
        let secret_path = "/home/alice/secret-project/src/payments.rs";
        let mut snap = Snapshot::new();
        let mut s = SeriesSnapshot {
            count: 3,
            sum_us: 30,
            max_us: 10,
            ..SeriesSnapshot::default()
        };
        s.histogram.record(10);
        snap.insert("db.get_node_by_id".into(), s.clone());
        let mut tool = s.clone();
        tool.repeated
            .insert("db.get_outgoing_edges".into(), Default::default());
        tool.repeated
            .insert("SELECT * FROM secret".into(), Default::default());
        snap.insert("tool.tokensave_search".into(), tool);
        snap.insert(secret_path.into(), s.clone());
        snap.insert("tool.charge_customer_card".into(), s.clone());
        let mut merged = BTreeMap::new();
        merged.insert("libsql".to_string(), snap.clone());
        merged.insert("secret-engine".to_string(), snap);

        let tools = BTreeSet::from(["tokensave_search".to_string()]);
        let export = sanitized(&merged, 24, &tools);
        let json = to_json(&export);
        for forbidden in [
            secret_path,
            "alice",
            "secret",
            "payments",
            "charge_customer_card",
        ] {
            assert!(!json.contains(forbidden), "export leaked {forbidden}");
        }
        assert!(json.contains("db.get_node_by_id"));
        assert!(json.contains("tool.tokensave_search"));
        assert!(json.contains("db.get_outgoing_edges"));
        assert!(json.contains(EXPORT_SCHEMA));
        let round_trip: Export = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip, export);
    }

    #[test]
    fn registry_tools_are_known() {
        let tools = known_tools();
        assert!(tools.contains("tokensave_search"));
        assert!(tools.contains(crate::profile::tools::OTHER));
    }
}
