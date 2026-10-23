//! Small deterministic, connected source fixture for profiling overhead smoke runs.
pub fn profile_leaf(value: usize) -> usize { value.wrapping_mul(3) }
pub fn profile_middle(value: usize) -> usize { profile_leaf(value) + profile_leaf(value + 1) }
pub fn profile_entry(value: usize) -> usize { profile_middle(value) + profile_leaf(value) }
pub fn profile_second(value: usize) -> usize { profile_entry(value) + profile_middle(value) }
pub fn profile_third(value: usize) -> usize { profile_second(value) + profile_entry(value) }
