pub trait Env {
    fn var(&self, key: &str) -> Option<String>;
}

pub struct SysEnv {}

impl SysEnv {
    pub fn new() -> Self {
        Self {}
    }
}

impl Env for SysEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashMap;

    pub struct MockEnv {
        envs: HashMap<String, String>,
    }

    impl MockEnv {
        pub fn new() -> Self {
            Self {
                envs: HashMap::new(),
            }
        }

        pub fn insert(&mut self, key: &str, value: &str) {
            self.envs.insert(key.into(), value.into());
        }
    }

    impl Env for MockEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.envs.get(key).map(|s| s.to_owned())
        }
    }
}
