use color_eyre::{eyre::eyre, Result};
use expectrl::{Session, WaitStatus};
use eyre::Context;
use std::{io::Read, mem, path::Path, time::Duration};

use crate::test_utils::base::TestBase;

pub struct TestHarness {
    base: TestBase,
}

pub struct RunningTestHarness {
    base: TestBase,
    session: Session,
}

pub struct TerminatedTestHarness {
    base: TestBase,
    wait_status: WaitStatus,
}

impl TestHarness {
    pub fn new() -> Self {
        Self {
            base: TestBase::new(),
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
    pub fn run(mut self, timeout_ms: u64) -> Result<RunningTestHarness> {
        let command = self.base.create_command()?;

        let mut session = Session::spawn(command).unwrap();
        session.set_expect_timeout(Some(Duration::from_millis(timeout_ms)));

        Ok(RunningTestHarness {
            base: TestBase {
                config_path: mem::take(&mut self.base.config_path),
                tmp_config_file: self.base.tmp_config_file.take(),
                name: mem::take(&mut self.base.name),
                args: mem::take(&mut self.base.args),
                working_dir: mem::take(&mut self.base.working_dir),
                envs: mem::take(&mut self.base.envs),
                command_string: mem::take(&mut self.base.command_string),
                replacements: mem::take(&mut self.base.replacements),
            },
            session,
        })
    }

    pub fn config_path(&self) -> &str {
        self.base.config_path()
    }

    pub fn name(&self) -> &str {
        self.base.name()
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl RunningTestHarness {
    #[track_caller]
    pub fn send_line(mut self, cmd: &str) -> Result<Self> {
        self.session
            .send_line(cmd)
            .wrap_err("Failed to send line")?;
        Ok(self)
    }

    #[track_caller]
    pub fn expect_string(mut self, expected: &str) -> Result<Self> {
        let mut parsed_expected = expected.trim().to_string();
        for (key, value) in &self.base.replacements {
            parsed_expected = parsed_expected.replace(key, value);
        }

        self.expect(parsed_expected)?;

        Ok(self)
    }

    #[track_caller]
    pub fn expect_terminate(mut self) -> Result<TerminatedTestHarness> {
        self.expect(&expectrl::Eof)?;

        let wait_status = self
            .session
            .get_process()
            .wait()
            .wrap_err("Failed to wait for process to exit")?;
        Ok(TerminatedTestHarness {
            base: TestBase {
                config_path: mem::take(&mut self.base.config_path),
                tmp_config_file: self.base.tmp_config_file.take(),
                name: mem::take(&mut self.base.name),
                args: mem::take(&mut self.base.args),
                working_dir: mem::take(&mut self.base.working_dir),
                envs: mem::take(&mut self.base.envs),
                command_string: mem::take(&mut self.base.command_string),
                replacements: mem::take(&mut self.base.replacements),
            },
            wait_status,
        })
    }

    #[must_use]
    pub fn config_path(&self) -> &str {
        self.base.config_path()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.base.name()
    }

    #[track_caller]
    fn expect<T: expectrl::Needle>(&mut self, expected: T) -> Result<()> {
        match self.session.expect(expected) {
            Ok(_) => (),
            Err(expectrl::Error::ExpectTimeout) => {
                let mut buf = [0; 1024];
                let mut buf_all = Vec::new();
                while let Ok(n) = self.session.try_read(&mut buf) {
                    buf_all.extend(&buf[..n]);
                }

                let string = String::from_utf8_lossy(&buf_all);

                panic!("Timeout Reached, Unexpected output:\n{}", string);
            }
            Err(expectrl::Error::Eof) => {
                let mut buf = String::new();
                let _ = self
                    .session
                    .read_to_string(&mut buf)
                    .wrap_err("Failed to read buf");
                panic!("Eof Reached, Unexpected output:\n{}", buf);
            }

            Err(e) => return Err(e).wrap_err("Failed to expect"),
        }

        Ok(())
    }
}

impl TerminatedTestHarness {
    #[track_caller]
    pub fn success(&self) -> Result<()> {
        match self.wait_status {
            WaitStatus::Exited(_, 0) => Ok(()),
            WaitStatus::Exited(_, n) => Err(eyre!("Unexpected exit code: {}", n)),
            v => Err(eyre!("Unexpected Process WaitStatus {:?}", v)),
        }
    }

    #[track_caller]
    pub fn failure(&self, expected_code: i32) -> Result<()> {
        match self.wait_status {
            WaitStatus::Exited(_, 0) => Err(eyre!("Unexpected successful exit")),
            WaitStatus::Exited(_, n) if n == expected_code => Ok(()),
            WaitStatus::Exited(_, n) => Err(eyre!("Unexpected exit code: {}", n)),
            v => Err(eyre!("Unexpected Process WaitStatus {:?}", v)),
        }
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
}
