//! Ambiguous references for the SCIP comparison fixture (#1100).
pub mod alpha;
pub mod beta;

use alpha::Config as AlphaConfig;

pub trait Validate {
    fn check(&self) -> bool;
}

pub fn run() -> bool {
    let a = alpha::Config::new();
    let b = beta::Config::new();
    let c: AlphaConfig = AlphaConfig::new();
    a.check() && b.check() && c.check() && beta::helper()
}

pub fn count() -> u32 {
    alpha::helper() + 1
}
