//! Static knowledge of what well-known Rust `#[derive(...)]` macros generate.
//!
//! The resolver cannot (and probably should not) follow `derive` expansion to
//! produce real impl/method nodes in the graph — the generated code lives in
//! `cargo expand` output that we don't run. To prevent agent dead-ends on
//! generated symbols (`my_struct.clone()`, `format!("{:?}", x)`, ...), this
//! table maps each well-known derive name to the trait it implements and the
//! method names that trait carries. Unknown derives (proc-macros from random
//! crates) still surface but with `methods: None`, so callers can render
//! "Foo derives `MyCustom`; methods unknown" rather than nothing at all.

use serde::Serialize;

/// What a derive macro contributes to a type's API surface.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DeriveInfo {
    /// Short derive name as written in source — e.g. `"Debug"`, `"Clone"`.
    pub derive_name: &'static str,
    /// Canonical trait path the derive implements.
    pub trait_path: &'static str,
    /// Method names the derived impl carries. Empty for marker traits
    /// (`Copy`, `Eq`).
    pub methods: &'static [&'static str],
    /// Crate the derive originates from (`std`, `serde`, ...). Useful so
    /// callers can tell at a glance whether the derive is built-in.
    pub source: &'static str,
}

/// Result of looking up a derive name in the table.
#[derive(Debug, Clone, Serialize)]
pub struct DeriveLookup {
    /// As parsed from the `#[derive(...)]` attribute.
    pub derive_name: String,
    /// Full info when the derive is in our well-known table; `None` for
    /// unknown / third-party proc-macro derives.
    pub known: Option<DeriveInfo>,
}

/// All well-known derives we surface today. Add to this list when new
/// derives become common enough that agents will hit dead-ends without them.
const WELL_KNOWN: &[DeriveInfo] = &[
    // std::fmt
    DeriveInfo {
        derive_name: "Debug",
        trait_path: "core::fmt::Debug",
        methods: &["fmt"],
        source: "std",
    },
    // core::clone / core::marker
    DeriveInfo {
        derive_name: "Clone",
        trait_path: "core::clone::Clone",
        methods: &["clone", "clone_from"],
        source: "std",
    },
    DeriveInfo {
        derive_name: "Copy",
        trait_path: "core::marker::Copy",
        methods: &[],
        source: "std",
    },
    // core::default
    DeriveInfo {
        derive_name: "Default",
        trait_path: "core::default::Default",
        methods: &["default"],
        source: "std",
    },
    // core::cmp
    DeriveInfo {
        derive_name: "PartialEq",
        trait_path: "core::cmp::PartialEq",
        methods: &["eq", "ne"],
        source: "std",
    },
    DeriveInfo {
        derive_name: "Eq",
        trait_path: "core::cmp::Eq",
        methods: &[],
        source: "std",
    },
    DeriveInfo {
        derive_name: "PartialOrd",
        trait_path: "core::cmp::PartialOrd",
        methods: &["partial_cmp", "lt", "le", "gt", "ge"],
        source: "std",
    },
    DeriveInfo {
        derive_name: "Ord",
        trait_path: "core::cmp::Ord",
        methods: &["cmp", "max", "min", "clamp"],
        source: "std",
    },
    // core::hash
    DeriveInfo {
        derive_name: "Hash",
        trait_path: "core::hash::Hash",
        methods: &["hash", "hash_slice"],
        source: "std",
    },
    // serde
    DeriveInfo {
        derive_name: "Serialize",
        trait_path: "serde::ser::Serialize",
        methods: &["serialize"],
        source: "serde",
    },
    DeriveInfo {
        derive_name: "Deserialize",
        trait_path: "serde::de::Deserialize",
        methods: &["deserialize"],
        source: "serde",
    },
    // Common marker traits commonly derived via proc-macro for completeness
    DeriveInfo {
        derive_name: "Display",
        trait_path: "core::fmt::Display",
        methods: &["fmt"],
        source: "derive_more / thiserror",
    },
    DeriveInfo {
        derive_name: "Error",
        trait_path: "std::error::Error",
        methods: &["source", "description", "cause"],
        source: "thiserror / std",
    },
];

/// Looks up a derive name. Returns `Some(DeriveInfo)` for well-known derives
/// and `None` for unknown / proc-macro derives.
pub fn lookup(derive_name: &str) -> Option<DeriveInfo> {
    WELL_KNOWN
        .iter()
        .find(|info| info.derive_name.eq_ignore_ascii_case(derive_name))
        .copied()
}

/// Wraps a derive name with whatever the table knows about it (or `None`).
pub fn enrich(derive_name: &str) -> DeriveLookup {
    DeriveLookup {
        derive_name: derive_name.to_string(),
        known: lookup(derive_name),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn debug_returns_fmt_method() {
        let info = lookup("Debug").expect("Debug is well-known");
        assert_eq!(info.trait_path, "core::fmt::Debug");
        assert_eq!(info.methods, &["fmt"]);
        assert_eq!(info.source, "std");
    }

    #[test]
    fn copy_is_marker_no_methods() {
        let info = lookup("Copy").expect("Copy is well-known");
        assert!(info.methods.is_empty());
    }

    #[test]
    fn unknown_derive_returns_none() {
        assert!(lookup("MyCustomProcMacro").is_none());
    }

    #[test]
    fn enrich_wraps_unknown_with_none() {
        let l = enrich("UnknownThing");
        assert_eq!(l.derive_name, "UnknownThing");
        assert!(l.known.is_none());
    }

    #[test]
    fn enrich_wraps_known() {
        let l = enrich("Clone");
        assert!(l.known.is_some());
        assert_eq!(l.known.unwrap().methods, &["clone", "clone_from"]);
    }
}
