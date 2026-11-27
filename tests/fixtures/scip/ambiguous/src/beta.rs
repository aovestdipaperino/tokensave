use crate::Validate;

pub struct Config {
    pub limit: u32,
}

impl Config {
    pub fn new() -> Self {
        Config { limit: 3 }
    }
}

impl Validate for Config {
    fn check(&self) -> bool {
        self.limit > crate::alpha::helper()
    }
}

pub fn helper() -> bool {
    true
}
