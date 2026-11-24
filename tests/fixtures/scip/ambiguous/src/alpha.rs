use crate::Validate;

pub struct Config {
    pub strict: bool,
}

impl Config {
    pub fn new() -> Self {
        Config { strict: true }
    }
}

impl Validate for Config {
    fn check(&self) -> bool {
        self.strict
    }
}

pub fn helper() -> u32 {
    1
}
