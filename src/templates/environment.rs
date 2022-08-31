use std::collections::HashMap;

pub trait Environment {
    fn resolve(&self, key: &str) -> Option<String>;
}

impl<'f> Environment for BorrowedEnv<'f> {
    fn resolve(&self, key: &str) -> Option<String> {
        self.map.get(key).map(|s| s.to_string())
    }
}

#[derive(Default)]
pub struct BorrowedEnv<'f> {
    map: HashMap<&'static str, &'f dyn std::fmt::Display>,
}

impl<'f> BorrowedEnv<'f> {
    pub fn insert(mut self, key: &'static str, val: &'f dyn std::fmt::Display) -> Self {
        self.map.insert(key, val);
        self
    }
}

pub trait RegisterResponse {
    fn register() -> anyhow::Result<()>;
}
