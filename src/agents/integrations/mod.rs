pub mod antigravity;
pub mod augment;
pub mod claude;
pub mod cline;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod droid;
pub mod gemini;
pub mod grok;
pub mod kilo;
pub mod kimi;
pub mod kiro;
pub mod omp;
pub mod opencode;
pub mod pi;
pub mod plank;
pub mod qwen;
pub mod roo_code;
pub mod vibe;
pub mod zed;

pub use antigravity::AntigravityIntegration;
pub use augment::AugmentIntegration;
pub use claude::ClaudeIntegration;
pub use cline::ClineIntegration;
pub use codex::CodexIntegration;
pub use copilot::CopilotIntegration;
pub use cursor::CursorIntegration;
pub use droid::DroidIntegration;
pub use gemini::GeminiIntegration;
pub use grok::GrokIntegration;
pub use kilo::KiloIntegration;
pub use kimi::KimiIntegration;
pub use kiro::KiroIntegration;
pub use omp::OmpIntegration;
pub use opencode::OpenCodeIntegration;
pub use pi::PiIntegration;
pub use plank::PlankIntegration;
pub use qwen::QwenIntegration;
pub use roo_code::RooCodeIntegration;
pub use vibe::VibeIntegration;
pub use zed::ZedIntegration;

use crate::agents::fs::{
    backup_and_write_json, backup_config_file, copilot_cli_dir, copilot_jetbrains_dir, home_dir,
    load_json_file, load_json_file_strict, load_jsonc_file, load_jsonc_file_strict, load_toml_file,
    parse_jsonc, resolve_symlink_target, safe_write_json_file, vscode_data_dir,
    vscode_insiders_data_dir, write_json_file, write_toml_file,
};
use crate::agents::rules::{
    check_managed_rules_file, check_shared_rules_block, remove_legacy_rules_block,
    remove_managed_rules_file, remove_rules_block, rules_for_agent, write_managed_rules_file,
    write_rules_block, LEGACY_RULES_MARKER, OMP_RULES_MARKER,
};
use crate::agents::traits::{
    AgentIntegration, DoctorCounters, HealthcheckContext, InstallContext, InstallScope,
};
use crate::agents::{expected_tool_perms, tool_names};
