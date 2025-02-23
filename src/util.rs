use std::{collections::HashMap, env};

#[derive(Debug)]
pub struct AppEnvVar {
    vars: HashMap<String, String>,
}

impl AppEnvVar {
    pub fn new() -> Self {
        AppEnvVar {
            vars: env::vars().collect(),
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

impl Default for AppEnvVar {
    fn default() -> Self {
        Self::new()
    }
}

pub trait UnexpectedExt<T> {
    fn unexpected(self) -> miette::Result<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> UnexpectedExt<T> for Result<T, E> {
    #[track_caller]
    fn unexpected(self) -> miette::Result<T> {
        let location = std::panic::Location::caller();
        let loc = format!("{}:{}", location.file(), location.line());
        self.map_err(move |e| {
            miette::miette!(
                code = "Unexpected Error, Please create issue on GitHub:",
                url = "https://github.com/ArchieAtkinson/berth/issues",
                "Unexpected error at {}: {}",
                loc,
                e
            )
        })
    }
}

impl<T> UnexpectedExt<T> for Option<T> {
    #[track_caller]
    fn unexpected(self) -> miette::Result<T> {
        let location = std::panic::Location::caller();
        let loc = format!("{}:{}", location.file(), location.line());
        self.ok_or_else(move || {
            miette::miette!(
                code = "Unexpected Error, Please create issue on GitHub:",
                url = "https://github.com/ArchieAtkinson/berth/issues",
                "Unexpected None at {}",
                loc
            )
        })
    }
}
