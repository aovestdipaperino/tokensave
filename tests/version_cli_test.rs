use std::process::Command;

#[test]
fn version_output_is_concise_and_verbose_metadata_needs_no_project() {
    let dir = tempfile::tempdir().expect("create empty working directory");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tokensave"))
            .current_dir(dir.path())
            .args(args)
            .output()
            .expect("run tokensave")
    };
    let plain = run(&["--version"]);
    assert!(plain.status.success());
    assert_eq!(
        plain.stdout,
        format!("tokensave {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
    for args in [["--version", "--verbose"], ["--verbose", "--version"]] {
        let output = run(&args);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).expect("UTF-8 output");
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], format!("tokensave {}", env!("CARGO_PKG_VERSION")));
        for (line, prefix) in lines[1..].iter().zip([
            "commit: ",
            "build-profile: ",
            "target: ",
            "rustc: ",
            // #1055: detected installer of the running executable.
            "install-method: ",
        ]) {
            assert!(!line
                .strip_prefix(prefix)
                .expect("metadata field")
                .is_empty());
        }
    }
    assert_eq!(
        std::fs::read_dir(dir.path())
            .expect("read working directory")
            .count(),
        0
    );
}
