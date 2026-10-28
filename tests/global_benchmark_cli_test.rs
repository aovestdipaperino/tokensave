//! The hardware benchmark is machine-level (#752): `tokensave init` measures
//! only when no valid global result is saved, every project reuses it, and
//! `tokensave config benchmark` manages it. Driven through the real binary
//! with a private home directory per test.
//!
//! Unix only: on Windows the profile directory is resolved without consulting
//! `HOME`/`USERPROFILE`, so a test could not keep the cache out of the real
//! profile (the same reason as `install_warning_test`).
#![cfg(not(windows))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;
use tokensave::config::configured_cpu_usage;

struct Machine {
    home: TempDir,
}

impl Machine {
    fn new() -> Self {
        Self {
            home: TempDir::new().unwrap(),
        }
    }

    fn global_dir(&self) -> PathBuf {
        self.home.path().join(".tokensave")
    }

    fn cache(&self) -> PathBuf {
        self.global_dir().join("benchmark.toml")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
        command
            .args(args)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("TOKENSAVE_CPU_USAGE")
            .stdin(Stdio::null());
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args)
            .output()
            .expect("failed to run tokensave")
    }

    fn init(&self, project: &Path) -> Output {
        self.command(&["init"])
            .arg(project)
            .output()
            .expect("failed to run tokensave")
    }

    fn write_global_config(&self, toml: &str) {
        std::fs::create_dir_all(self.global_dir()).unwrap();
        std::fs::write(self.global_dir().join("config.toml"), toml).unwrap();
    }

    /// Saves a valid result for this machine that recommends `profile`, so a
    /// test can tell the benchmark layer apart from the built-in default.
    fn seed_cache(&self, profile: &str) {
        let r = tokensave::hardware::probe_resources();
        let mut text = format!(
            "schema_version = {}\nalgorithm_version = {}\nmeasured_at = 0\n\
             tokensave_version = \"test\"\n\n[recommendation]\nprofile = \"{profile}\"\n\
             reason = \"seeded by a test\"\n\n[measurement.resources]\n",
            tokensave::global_benchmark::SCHEMA_VERSION,
            tokensave::hardware::ALGORITHM_VERSION
        );
        if let Some(cpus) = r.cpus {
            text.push_str(&format!("cpus = {cpus}\n"));
        }
        if let Some(total) = r.memory_total {
            text.push_str(&format!("memory_total = {total}\n"));
        }
        text.push_str("\n[measurement.benchmark]\nthreads = 1\nspeedup = 1.0\nelapsed_ms = 1\n");
        std::fs::create_dir_all(self.global_dir()).unwrap();
        std::fs::write(self.cache(), text).unwrap();
    }
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "pub fn hello() {}\n").unwrap();
    dir
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("stdout is JSON ({e}): {}", combined(output)))
}

#[test]
fn first_init_measures_and_later_projects_reuse_the_result() {
    let machine = Machine::new();
    let (a, b, c) = (project(), project(), project());

    let first = machine.init(a.path());
    let text = combined(&first);
    assert!(first.status.success(), "{text}");
    assert!(
        text.contains("No global hardware benchmark found."),
        "{text}"
    );
    assert!(text.contains("Global benchmark results saved"), "{text}");
    assert!(
        text.contains("(from the global hardware benchmark)"),
        "{text}"
    );
    let saved = std::fs::read(machine.cache()).unwrap();

    for later in [b.path(), c.path()] {
        let output = machine.init(later);
        let text = combined(&output);
        assert!(output.status.success(), "{text}");
        assert!(
            text.contains("Using cached global hardware benchmark."),
            "{text}"
        );
        assert!(!text.contains("Running"), "{text}");
        assert!(
            text.contains("(from the global hardware benchmark)"),
            "{text}"
        );
    }
    // One cache for the machine, untouched by the later inits, and no
    // machine-level data copied into any project.
    assert_eq!(std::fs::read(machine.cache()).unwrap(), saved);
    for dir in [a.path(), b.path(), c.path()] {
        assert_eq!(configured_cpu_usage(dir).unwrap(), None);
        assert!(!dir.join(".tokensave").join("benchmark.toml").exists());
    }
}

#[test]
fn a_valid_saved_result_is_used_without_measuring() {
    let machine = Machine::new();
    machine.seed_cache("performance");
    let dir = project();
    let output = machine.init(dir.path());
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("Using cached global hardware benchmark."));
    assert!(
        text.contains("CPU usage profile: performance (from the global hardware benchmark)"),
        "{text}"
    );
}

#[test]
fn an_invalid_saved_result_is_measured_again() {
    let machine = Machine::new();
    std::fs::create_dir_all(machine.global_dir()).unwrap();
    std::fs::write(machine.cache(), "schema_version = 999\n").unwrap();
    let dir = project();
    let text = combined(&machine.init(dir.path()));
    assert!(
        text.contains("can't be reused: the file uses unsupported schema version 999"),
        "{text}"
    );
    assert!(text.contains("Global benchmark results saved"), "{text}");
    let show = json(&machine.run(&["config", "benchmark", "--show", "--json"]));
    assert_eq!(show["status"], "valid", "{show}");
}

#[test]
fn explicit_settings_beat_the_benchmark() {
    let machine = Machine::new();
    machine.seed_cache("performance");

    let pinned = project();
    let output = machine
        .command(&["init", "--cpu-usage", "low"])
        .arg(pinned.path())
        .output()
        .unwrap();
    let text = combined(&output);
    assert!(
        text.contains("CPU usage profile: low (from --cpu-usage)"),
        "{text}"
    );
    assert!(!text.contains("benchmark"), "{text}");

    let from_env = project();
    let output = machine
        .command(&["init"])
        .arg(from_env.path())
        .env("TOKENSAVE_CPU_USAGE", "balanced")
        .output()
        .unwrap();
    let text = combined(&output);
    assert!(
        text.contains("CPU usage profile: balanced (from TOKENSAVE_CPU_USAGE)"),
        "{text}"
    );
    assert!(!text.contains("benchmark"), "{text}");

    // The environment also beats a project pin, per run.
    let output = machine
        .command(&["config", "benchmark", "--show", "--json"])
        .env("TOKENSAVE_CPU_USAGE", "low")
        .output()
        .unwrap();
    assert_eq!(json(&output)["effective"]["source"], "environment");

    machine.write_global_config("cpu_usage = \"low\"\n");
    let global = project();
    let text = combined(&machine.init(global.path()));
    assert!(
        text.contains("CPU usage profile: low (from ~/.tokensave/config.toml)"),
        "{text}"
    );
    assert!(!text.contains("benchmark"), "{text}");
}

#[test]
fn a_disabled_benchmark_never_runs_and_is_never_used() {
    let machine = Machine::new();
    machine.write_global_config("[benchmark]\nenabled = false\n");
    let dir = project();
    let output = machine.init(dir.path());
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("CPU usage profile: balanced (built-in default)"),
        "{text}"
    );
    assert!(!text.contains("benchmark"), "{text}");
    assert!(!machine.cache().exists());

    machine.seed_cache("performance");
    let show = json(&machine.run(&["config", "benchmark", "--show", "--json"]));
    assert_eq!(show["effective"]["source"], "default", "{show}");

    let output = machine.run(&["config", "benchmark"]);
    assert_eq!(output.status.code(), Some(1), "{}", combined(&output));
    assert!(combined(&output).contains("disabled"));
}

#[test]
fn auto_run_off_waits_for_an_explicit_benchmark() {
    let machine = Machine::new();
    machine.write_global_config("[benchmark]\nauto_run = false\n");
    let first = project();
    let text = combined(&machine.init(first.path()));
    assert!(text.contains("auto_run = false"), "{text}");
    assert!(text.contains("(built-in default)"), "{text}");
    assert!(!machine.cache().exists());

    let output = machine.run(&["config", "benchmark"]);
    assert!(output.status.success(), "{}", combined(&output));
    assert!(machine.cache().exists());

    let second = project();
    let text = combined(&machine.init(second.path()));
    assert!(
        text.contains("Using cached global hardware benchmark."),
        "{text}"
    );
    // The setting survives untouched.
    let config = std::fs::read_to_string(machine.global_dir().join("config.toml")).unwrap();
    assert!(config.contains("auto_run = false"), "{config}");
}

#[test]
fn init_falls_back_to_the_default_when_the_result_cannot_be_saved() {
    let parent = TempDir::new().unwrap();
    let home_file = parent.path().join("home");
    std::fs::write(&home_file, "").unwrap();
    let dir = project();
    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("init")
        .arg(dir.path())
        .env("HOME", &home_file)
        .env("USERPROFILE", &home_file)
        .env_remove("TOKENSAVE_CPU_USAGE")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("could not save the hardware benchmark"),
        "{text}"
    );
    assert!(
        text.contains("CPU usage profile: balanced (built-in default)"),
        "{text}"
    );
    assert!(tokensave::config::get_tokensave_dir(dir.path())
        .join("tokensave.db")
        .exists());
}

#[test]
fn config_benchmark_runs_reuses_forces_shows_and_resets() {
    let machine = Machine::new();

    // --show never measures, even with nothing saved.
    let show = machine.run(&["config", "benchmark", "--show", "--json"]);
    assert!(show.status.success());
    assert_eq!(json(&show)["status"], "missing");
    assert!(!machine.cache().exists());

    let run = machine.run(&["config", "benchmark", "--json"]);
    assert!(run.status.success(), "{}", combined(&run));
    let report = json(&run);
    assert_eq!(report["status"], "measured");
    assert_eq!(report["saved"], true);
    assert!(report["recommended_workers"].as_u64() >= Some(1));
    assert!(machine.cache().exists());

    let again = json(&machine.run(&["config", "benchmark", "--json"]));
    assert_eq!(again["status"], "cached");

    let forced = machine.run(&["config", "benchmark", "--force"]);
    assert!(forced.status.success());
    assert!(combined(&forced).contains("Running hardware benchmark (--force)"));

    let show = machine.run(&["config", "benchmark", "--show"]);
    let text = combined(&show);
    assert!(text.contains("valid for this machine"), "{text}");
    assert!(text.contains("Recommended:"), "{text}");

    let reset = json(&machine.run(&["config", "benchmark", "--reset", "--json"]));
    assert_eq!(reset["removed"], true);
    assert!(!machine.cache().exists());
    let reset = json(&machine.run(&["config", "benchmark", "--reset", "--json"]));
    assert_eq!(reset["removed"], false);

    // After a reset the next init measures again.
    let dir = project();
    let text = combined(&machine.init(dir.path()));
    assert!(
        text.contains("No global hardware benchmark found."),
        "{text}"
    );
}

#[test]
fn config_benchmark_rejects_conflicting_flags() {
    let machine = Machine::new();
    for args in [
        ["config", "benchmark", "--force", "--show"],
        ["config", "benchmark", "--show", "--reset"],
        ["config", "benchmark", "--force", "--reset"],
    ] {
        let output = machine.run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(
            combined(&output).contains("cannot be used with"),
            "{args:?}"
        );
    }
    assert!(!machine.cache().exists());
}

#[test]
fn unrelated_global_settings_survive_benchmark_management() {
    let machine = Machine::new();
    let original = "upload_enabled = false\nwildcard_permissions = true\n";
    machine.write_global_config(original);
    assert!(machine.run(&["config", "benchmark"]).status.success());
    assert!(machine
        .run(&["config", "benchmark", "--reset"])
        .status
        .success());
    let config = std::fs::read_to_string(machine.global_dir().join("config.toml")).unwrap();
    assert!(config.contains("upload_enabled = false"), "{config}");
    assert!(config.contains("wildcard_permissions = true"), "{config}");
}

#[test]
fn simultaneous_inits_share_one_benchmark() {
    let machine = Machine::new();
    let projects: Vec<TempDir> = (0..4).map(|_| project()).collect();
    let children: Vec<_> = projects
        .iter()
        .map(|p| {
            machine
                .command(&["init"])
                .arg(p.path())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let outputs: Vec<Output> = children
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect();
    let mut measured = 0;
    for output in &outputs {
        let text = combined(output);
        assert!(output.status.success(), "{text}");
        assert!(
            text.contains("(from the global hardware benchmark)"),
            "{text}"
        );
        if text.contains("Running initial hardware benchmark") {
            measured += 1;
        }
    }
    assert_eq!(
        measured, 1,
        "exactly one process measures; the rest reuse it"
    );
    let show = json(&machine.run(&["config", "benchmark", "--show", "--json"]));
    assert_eq!(show["status"], "valid");
}
