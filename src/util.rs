use std::{collections::HashMap, env};

#[derive(Debug)]
pub struct AppEnvVar {
    vars: HashMap<String, String>,
}

impl AppEnvVar {
    pub fn new() -> Self {
        AppEnvVar {
            vars: env::vars().into_iter().collect(),
        }
    }

    pub fn set_var(mut self, var: &str, value: &str) -> Self {
        self.vars.insert(var.to_string(), value.to_string());
        self
    }

    pub fn var(&self, var: &str) -> Option<&str> {
        self.vars.get(var).map(|v| v.as_str())
    }
}
