use tokensave::derive_table::{enrich, lookup};

#[test]
fn all_well_known_derives_are_found() {
    let expected = &[
        "Debug",
        "Clone",
        "Copy",
        "Default",
        "PartialEq",
        "Eq",
        "PartialOrd",
        "Ord",
        "Hash",
        "Serialize",
        "Deserialize",
        "Display",
        "Error",
    ];
    for name in expected {
        assert!(
            lookup(name).is_some(),
            "expected {} to be in the well-known table",
            name
        );
    }
}

#[test]
fn lookup_is_case_insensitive() {
    assert!(lookup("debug").is_some(), "lowercase should match");
    assert!(lookup("DEBUG").is_some(), "uppercase should match");
    assert!(lookup("DeBuG").is_some(), "mixed case should match");
    assert!(lookup("clone").is_some(), "lowercase Clone should match");
    assert!(lookup("COPY").is_some(), "uppercase Copy should match");
}

#[test]
fn partial_ord_methods() {
    let info = lookup("PartialOrd").expect("PartialOrd is well-known");
    assert_eq!(info.trait_path, "core::cmp::PartialOrd");
    assert!(info.methods.contains(&"partial_cmp"));
    assert!(info.methods.contains(&"lt"));
    assert!(info.methods.contains(&"le"));
    assert!(info.methods.contains(&"gt"));
    assert!(info.methods.contains(&"ge"));
}

#[test]
fn ord_methods() {
    let info = lookup("Ord").expect("Ord is well-known");
    assert_eq!(info.trait_path, "core::cmp::Ord");
    assert!(info.methods.contains(&"cmp"));
    assert!(info.methods.contains(&"max"));
    assert!(info.methods.contains(&"min"));
    assert!(info.methods.contains(&"clamp"));
}

#[test]
fn hash_methods() {
    let info = lookup("Hash").expect("Hash is well-known");
    assert_eq!(info.trait_path, "core::hash::Hash");
    assert_eq!(info.source, "std");
    assert!(info.methods.contains(&"hash"));
    assert!(info.methods.contains(&"hash_slice"));
}

#[test]
fn serde_derives_correct_source() {
    let serialize = lookup("Serialize").expect("Serialize is well-known");
    assert_eq!(serialize.source, "serde");
    assert_eq!(serialize.trait_path, "serde::ser::Serialize");

    let deserialize = lookup("Deserialize").expect("Deserialize is well-known");
    assert_eq!(deserialize.source, "serde");
    assert_eq!(deserialize.trait_path, "serde::de::Deserialize");
}

#[test]
fn display_and_error_not_from_std_core() {
    let display = lookup("Display").expect("Display is well-known");
    assert_eq!(display.trait_path, "core::fmt::Display");
    assert_ne!(display.source, "std");

    let error = lookup("Error").expect("Error is well-known");
    assert_eq!(error.trait_path, "std::error::Error");
}

#[test]
fn each_well_known_has_derive_name_matching_lookup() {
    // Verify consistency: lookup(name).derive_name == name
    let names = [
        "Debug",
        "Clone",
        "Copy",
        "Default",
        "PartialEq",
        "Eq",
        "PartialOrd",
        "Ord",
        "Hash",
        "Serialize",
        "Deserialize",
        "Display",
        "Error",
    ];
    for name in names {
        let info = lookup(name).unwrap_or_else(|| panic!("{name} not found"));
        // Case-insensitive, so info.derive_name preserves canonical casing
        assert!(
            info.derive_name.eq_ignore_ascii_case(name),
            "derive_name {} does not match lookup {}",
            info.derive_name,
            name
        );
    }
}

#[test]
fn enrich_preserves_original_casing() {
    let l = enrich("debug");
    assert_eq!(l.derive_name, "debug");
    assert!(l.known.is_some());
    // The known info keeps canonical casing
    assert_eq!(l.known.unwrap().derive_name, "Debug");
}

#[test]
fn enrich_unknown_preserves_exact_input() {
    let l = enrich(" MyWeirdMacro ");
    assert_eq!(l.derive_name, " MyWeirdMacro ");
    assert!(l.known.is_none());
}

#[test]
fn lookup_empty_string() {
    assert!(lookup("").is_none());
}

#[test]
fn marker_traits_have_no_methods() {
    for name in &["Copy", "Eq"] {
        let info = lookup(name).unwrap_or_else(|| panic!("{name} is well-known"));
        assert!(
            info.methods.is_empty(),
            "{} should be a marker trait with no methods",
            name
        );
    }
}

#[test]
fn non_marker_traits_have_methods() {
    for name in &[
        "Debug",
        "Clone",
        "Default",
        "PartialEq",
        "PartialOrd",
        "Ord",
        "Hash",
        "Serialize",
        "Deserialize",
        "Display",
        "Error",
    ] {
        let info = lookup(name).unwrap_or_else(|| panic!("{name} is well-known"));
        assert!(!info.methods.is_empty(), "{} should have methods", name);
    }
}
