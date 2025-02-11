use color_eyre::{eyre::eyre, Result};
use eyre::{Context, ContextCompat};
use pretty_assertions::assert_eq;
use std::path::Path;

use crate::test_utils::base::TestBase;

pub struct TestOutput {
    base: TestBase,
    stdout: String,
    stderr: String,
    exit_code: i32,
}

impl TestOutput {
    pub fn new() -> Self {
        Self {
            base: TestBase::new(),
            stdout: String::new(),
            stderr: String::new(),
            exit_code: -1,
        }
    }

    #[track_caller]
    pub fn config(mut self, content: &str) -> Result<Self> {
        self.base.config(content)?;
        Ok(self)
    }

    #[track_caller]
    pub fn config_with_path(mut self, content: &str, path: &Path) -> Result<Self> {
        self.base.config_with_path(content, path)?;
        Ok(self)
    }

    #[track_caller]
    pub fn args(mut self, args: Vec<&str>) -> Result<Self> {
        self.base.args(args)?;
        Ok(self)
    }

    #[track_caller]
    pub fn envs(mut self, envs: Vec<(&str, &str)>) -> Result<Self> {
        self.base.envs(envs)?;
        Ok(self)
    }

    #[track_caller]
    pub fn working_dir(mut self, working_dir: &str) -> Result<Self> {
        self.base.working_dir(working_dir)?;
        Ok(self)
    }

    #[track_caller]
    pub fn stdout(mut self, content: impl Into<String>) -> Result<Self> {
        self.stdout = content.into();
        for (key, value) in &self.base.replacements {
            self.stdout = self.stderr.replace(key, value);
        }

        Ok(self)
    }

    #[track_caller]
    pub fn stderr(mut self, content: impl Into<String>) -> Result<Self> {
        self.stderr = content.into();
        for (key, value) in &self.base.replacements {
            self.stderr = self.stderr.replace(key, value);
        }

        Ok(self)
    }

    #[track_caller]
    pub fn code(mut self, code: i32) -> Result<Self> {
        self.exit_code = code;
        Ok(self)
    }

    #[must_use]
    #[track_caller]
    pub fn config_path(&self) -> &str {
        self.base.config_path()
    }

    #[must_use]
    #[track_caller]
    pub fn name(&self) -> &str {
        self.base.name()
    }

    #[track_caller]
    pub fn run(&mut self) -> Result<()> {
        let output = self
            .base
            .create_command()?
            .output()
            .wrap_err(eyre!("Failed to run {}", self.base.command_string))?;
        let output_stdout =
            String::from_utf8(output.stdout).wrap_err("Failed to convert stdout from utf8")?;
        let output_stderr =
            String::from_utf8(output.stderr).wrap_err("Failed to convert stderr from utf8")?;
        let output_exit_code = output.status.code().wrap_err("Failed to get exit code")?;

        assert_eq!(output_stdout, self.stdout);
        assert_eq!(output_stderr, self.stderr);
        assert_eq!(output_exit_code, self.exit_code);

        Ok(())
    }
}

impl Default for TestOutput {
    fn default() -> Self {
        Self::new()
    }
}
