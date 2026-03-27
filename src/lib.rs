#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::similar_names)]
#![allow(clippy::wildcard_imports)]

pub mod agents;
pub mod cloud;
pub mod daemon;
pub mod config;
pub mod context;
pub mod db;
pub mod display;
pub mod doctor;
pub mod errors;
pub mod extraction;
pub mod global_db;
pub mod graph;
pub mod mcp;
pub mod resolution;
pub mod sync;
pub mod tokensave;
pub mod tree_sitter;
pub mod types;
pub mod user_config;
pub mod vectors;
