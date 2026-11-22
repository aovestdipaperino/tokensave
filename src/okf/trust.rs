//! Trust tiers, freshness and actors (OKF v0.3 §5.2, §5.3, §5.5, §7).
//!
//! Where the specification leaves a case open, this follows the reference
//! implementation of the specification repository: a `generated` without
//! `at` keeps every verification current, and a `generated.at` that cannot
//! be parsed makes none current.

use serde::Serialize;
use serde_json::Value;

use super::consumer::items;
use super::consumer::present;
use super::consumer::ReadMode;
use super::time;

/// A trust tier, lowest first (§5.3). Tiers are advisory signals, not
/// access control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    Unverified,
    MachineConfirmed,
    HumanReviewed,
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unverified => "unverified",
            Self::MachineConfirmed => "machine-confirmed",
            Self::HumanReviewed => "human-reviewed",
        })
    }
}

/// One usable `verified` event: it has both `by` and a parseable `at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verification {
    pub by: String,
    pub at: String,
    #[serde(skip)]
    pub instant: i64,
}

/// The trust state of a concept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trust {
    pub tier: Tier,
    /// Verifications at or after the last meaningful change.
    pub current: Vec<Verification>,
    /// Verifications before the last meaningful change: history only, never
    /// approval of the current content.
    pub earlier: Vec<Verification>,
    /// `verified` entries without `by` or a parseable `at`, which consumers
    /// ignore.
    pub ignored: usize,
}

/// Whether an actor is a person: the `human:` scheme with an id (§7).
pub fn is_human(actor: &str) -> bool {
    actor
        .strip_prefix("human:")
        .is_some_and(|id| !id.trim().is_empty())
}

/// Whether `actor` has one of the two actor forms of §7:
/// `<producer>/<version>` or `<scheme>:<id>`.
pub fn is_actor(actor: &str) -> bool {
    let actor = actor.trim();
    if let Some((scheme, id)) = actor.split_once(':') {
        let mut chars = scheme.chars();
        let scheme_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | '_'));
        if scheme_ok && !id.trim().is_empty() {
            return true;
        }
    }
    match actor.split_once('/') {
        Some((producer, version)) => {
            !producer.is_empty() && !version.is_empty() && !actor.chars().any(char::is_whitespace)
        }
        None => false,
    }
}

/// Parses an OKF timestamp value (§5); only strings carry one, since the
/// YAML loader keeps native timestamps as text.
pub fn timestamp(value: &Value) -> Option<i64> {
    value.as_str().and_then(time::parse_timestamp)
}

/// The instant of the last meaningful change, and whether one is declared
/// but cannot be parsed. In [`ReadMode::Legacy`], a v0.1 `timestamp` stands
/// in for an absent `generated` (Appendix B.1).
fn generated_at(frontmatter: &Value, mode: ReadMode) -> Result<Option<i64>, ()> {
    let at = match present(frontmatter, "generated") {
        Some(Value::Object(generated)) => generated.get("at").filter(|v| !v.is_null()),
        None if mode.legacy() => present(frontmatter, "timestamp"),
        Some(_) | None => None,
    };
    match at {
        None => Ok(None),
        Some(value) => timestamp(value).map(Some).ok_or(()),
    }
}

/// Computes the trust tier of a concept from its current verifications.
pub fn assess(frontmatter: &Value, mode: ReadMode) -> Trust {
    let mut events = Vec::new();
    let mut ignored = 0usize;
    if let Some(verified) = present(frontmatter, "verified") {
        for event in items(verified) {
            let by = event.get("by").and_then(Value::as_str).map(str::trim);
            let at = event
                .get("at")
                .and_then(|at| Some((at.as_str()?, timestamp(at)?)));
            match (by, at) {
                (Some(by), Some((text, instant))) if !by.is_empty() => events.push(Verification {
                    by: by.to_string(),
                    at: text.to_string(),
                    instant,
                }),
                _ => ignored += 1,
            }
        }
    }
    let (current, earlier): (Vec<_>, Vec<_>) = match generated_at(frontmatter, mode) {
        Ok(None) => (events, Vec::new()),
        Ok(Some(changed)) => events.into_iter().partition(|e| e.instant >= changed),
        // A change time that cannot be read cannot be compared.
        Err(()) => (Vec::new(), events),
    };
    let tier = if current.iter().any(|e| is_human(&e.by)) {
        Tier::HumanReviewed
    } else if current.is_empty() {
        Tier::Unverified
    } else {
        Tier::MachineConfirmed
    };
    Trust {
        tier,
        current,
        earlier,
        ignored,
    }
}

/// Whether a concept is due for review (§5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// No `stale_after`: no review date is known.
    Unknown,
    Fresh,
    /// `now >= stale_after`: someone must check it again; it is not false.
    Stale,
    /// `stale_after` cannot be parsed, so it is never treated as fresh.
    Malformed,
}

/// The freshness of a concept at `now` (Unix seconds), from `stale_after`
/// only.
pub fn freshness(frontmatter: &Value, now: i64) -> Freshness {
    match present(frontmatter, "stale_after") {
        None => Freshness::Unknown,
        Some(value) => match timestamp(value) {
            None => Freshness::Malformed,
            Some(instant) if now >= instant => Freshness::Stale,
            Some(_) => Freshness::Fresh,
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tier(fm: &Value) -> Tier {
        assess(fm, ReadMode::Current).tier
    }

    #[test]
    fn only_current_verifications_count() {
        let reviewed = json!({
            "generated": {"by": "agent/1", "at": "2026-06-20T00:00:00Z"},
            "verified": {"by": "human:ana", "at": "2026-06-25T00:00:00Z"},
        });
        assert_eq!(tier(&reviewed), Tier::HumanReviewed);
        // An agent rewrote the concept after the human review.
        let rewritten = json!({
            "generated": {"by": "agent/1", "at": "2026-06-30T00:00:00Z"},
            "verified": [
                {"by": "human:ana", "at": "2026-06-25T00:00:00Z"},
                {"by": "process:nightly", "at": "2026-07-01T00:00:00Z"},
            ],
        });
        let trust = assess(&rewritten, ReadMode::Current);
        assert_eq!(trust.tier, Tier::MachineConfirmed);
        assert_eq!(trust.earlier.len(), 1);
        assert_eq!(trust.earlier[0].by, "human:ana");
        // A verification at the same instant as the change is current.
        let same = json!({
            "generated": {"by": "agent/1", "at": "2026-06-25"},
            "verified": {"by": "human:ana", "at": "2026-06-25T00:00:00Z"},
        });
        assert_eq!(tier(&same), Tier::HumanReviewed);
    }

    #[test]
    fn teams_and_unknown_schemes_are_not_human() {
        let fm = json!({"verified": [
            {"by": "team:fpa", "at": "2026-06-25T00:00:00Z"},
            {"by": "human:", "at": "2026-06-25T00:00:00Z"},
            {"by": "humans:x", "at": "2026-06-25T00:00:00Z"},
        ]});
        assert_eq!(tier(&fm), Tier::MachineConfirmed);
    }

    #[test]
    fn missing_or_bad_timestamps() {
        // No `generated`: every complete event is current.
        let fm = json!({"verified": {"by": "human:a", "at": "2026-01-01"}});
        assert_eq!(tier(&fm), Tier::HumanReviewed);
        // `generated` without `at`: as if absent (reference implementation).
        let fm =
            json!({"generated": {"by": "x/1"}, "verified": {"by": "human:a", "at": "2026-01-01"}});
        assert_eq!(tier(&fm), Tier::HumanReviewed);
        // An unreadable `generated.at` cannot be compared: nothing is current.
        let fm = json!({"generated": {"by": "x/1", "at": "yesterday"}, "verified": {"by": "human:a", "at": "2026-01-01"}});
        let trust = assess(&fm, ReadMode::Current);
        assert_eq!((trust.tier, trust.earlier.len()), (Tier::Unverified, 1));
        // Incomplete events are ignored.
        let fm = json!({"verified": [{"by": "human:a"}, {"at": "2026-01-01"}, {"by": "human:b", "at": "soon"}, "x"]});
        let trust = assess(&fm, ReadMode::Current);
        assert_eq!((trust.tier, trust.ignored), (Tier::Unverified, 4));
        // Empty values are absent.
        assert_eq!(
            tier(&json!({"verified": [], "generated": null})),
            Tier::Unverified
        );
    }

    #[test]
    fn legacy_timestamp_stands_in_for_generated() {
        let fm = json!({
            "timestamp": "2026-06-30T00:00:00Z",
            "verified": {"by": "human:a", "at": "2026-06-01T00:00:00Z"},
        });
        assert_eq!(assess(&fm, ReadMode::Legacy).tier, Tier::Unverified);
        assert_eq!(assess(&fm, ReadMode::Current).tier, Tier::HumanReviewed);
    }

    #[test]
    fn freshness_comes_from_stale_after_only() {
        let at = time::parse_timestamp("2026-09-23T00:00:00Z").unwrap_or_default();
        let fm = json!({"stale_after": "2026-09-23T00:00:00Z"});
        assert_eq!(freshness(&fm, at - 1), Freshness::Fresh);
        assert_eq!(freshness(&fm, at), Freshness::Stale);
        assert_eq!(
            freshness(&json!({"stale_after": "soon"}), 0),
            Freshness::Malformed
        );
        assert_eq!(
            freshness(&json!({"stale_after": 5}), 0),
            Freshness::Malformed
        );
        assert_eq!(
            freshness(&json!({"stale_after": null}), 0),
            Freshness::Unknown
        );
        assert_eq!(
            freshness(&json!({"stale_after": ""}), 0),
            Freshness::Unknown
        );
    }

    #[test]
    fn actor_forms() {
        for actor in [
            "reference_agent/gemini-2.5-pro",
            "human:ahormati",
            "team:finance-fpa",
            "process:finance-nightly",
            "acme-bot:42",
        ] {
            assert!(is_actor(actor), "{actor}");
        }
        for actor in ["Google", "human:", "/x", "agent/", "Jane Doe/1", ""] {
            assert!(!is_actor(actor), "{actor}");
        }
    }
}
