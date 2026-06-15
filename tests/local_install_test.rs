use tokensave::agents::get_integration;

#[test]
fn codex_does_not_support_local() {
    let ag = get_integration("codex").unwrap();
    assert!(!ag.supports_local(), "codex must remain global-only");
}
