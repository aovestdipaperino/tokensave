//! #526 follow-up: HTTPS must keep working on a host with no CA store.
//!
//! `RootCerts::PlatformVerifier` *replaces* ureq's bundled Mozilla roots
//! rather than adding to them, and on Linux/BSD `rustls-platform-verifier`
//! returns `No CA certificates were loaded from the system` when the store
//! yields nothing — with no fallback. So a distroless or `scratch` container
//! without `ca-certificates`, or a minimal CI image, would go from working on
//! the bundled roots to having no HTTPS at all.
//!
//! tokensave probes the store first and keeps the bundled roots when there is
//! nothing there. This pins the probe: without it the fallback is never taken
//! and the outage ships.
//!
//! One test per file on purpose — it sets `SSL_CERT_FILE`/`SSL_CERT_DIR`,
//! which are process-global, and cargo runs a file's tests as threads in one
//! binary.
//!
//! Linux/BSD only, which is where the failure mode exists; macOS and Windows
//! query OS APIs that cannot come back empty this way.
#![cfg(all(unix, not(target_vendor = "apple"), not(target_os = "android")))]

/// An empty trust store must be *detected* as empty. If this regressed, the
/// probe would report roots that are not there, tokensave would hand ureq the
/// platform verifier, and every HTTPS call would fail on exactly the hosts
/// this fallback exists for.
#[test]
fn an_empty_trust_store_is_detected_so_the_bundled_roots_are_kept() {
    let dir = tempfile::Builder::new()
        .prefix("tokensave_no_certs_")
        .tempdir()
        .expect("temp dir");

    // A valid, readable, empty PEM bundle: the store exists and simply holds
    // no certificates, which is the shape a minimal container has.
    let empty_bundle = dir.path().join("empty.pem");
    std::fs::write(&empty_bundle, "").expect("write empty bundle");

    // `rustls-native-certs` honours these, and is the same loader
    // `rustls-platform-verifier` uses on this target — so the probe is
    // answering the question the verifier is about to ask.
    std::env::set_var("SSL_CERT_FILE", &empty_bundle);
    std::env::set_var("SSL_CERT_DIR", dir.path());

    let available = tokensave::cloud::probe_platform_roots();

    std::env::remove_var("SSL_CERT_FILE");
    std::env::remove_var("SSL_CERT_DIR");

    assert!(
        !available,
        "an empty trust store must report no platform roots, so the bundled \
         Mozilla roots are used instead of a verifier that cannot be built"
    );
}
