use color_eyre::{eyre::eyre, Result};
use ctor::ctor;
use eyre::{Context, ContextCompat};
use pretty_assertions::assert_eq;
use rand::{thread_rng, Rng};
use rexpect::{
    process::wait::WaitStatus,
    session::{spawn_command, PtySession},
    ReadUntil,
};
use std::{
    env, fs, mem,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;

#[ctor]
fn setup() {
    color_eyre::install().unwrap();
}

pub struct TmpEnvVar {
    name: String,
    value: String,
}

impl TmpEnvVar {
    pub fn new(value: &str) -> TmpEnvVar {
        let name = Self::generate_env_var_name();
        env::set_var(name.clone(), value);
        assert_ne!(&name, value);

        TmpEnvVar {
            name,
            value: value.to_string(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    fn generate_env_var_name() -> String {
        const LENGTH: usize = 32;
        let mut rng = thread_rng();

        let chars: Vec<char> = (b'a'..=b'z').chain(b'A'..=b'Z').map(char::from).collect();

        (0..LENGTH)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect()
    }
}

impl Drop for TmpEnvVar {
    fn drop(&mut self) {
        env::remove_var(&self.name)
    }
}

const BINARY: &str = env!("CARGO_PKG_NAME");

struct TestBase {
    config_file: PathBuf,
    tmp_file: Option<NamedTempFile>,
    name: String,
    args: Vec<String>,
    working_dir: Option<PathBuf>,
    envs: Vec<(String, String)>,
    command_string: String,
}

impl TestBase {
    pub fn new() -> Self {
        Self {
            config_file: PathBuf::new(),
            tmp_file: None,
            name: Self::generate_random_environment_name(),
            args: Vec::new(),
            working_dir: None,
            envs: Vec::new(),
            command_string: String::new(),
        }
    }

    #[track_caller]
    pub fn config(&mut self, content: &str) -> Result<&mut Self> {
        let tmp_file =
            NamedTempFile::new().wrap_err("Failed to create temporary file for config")?;
        let path = tmp_file.path().to_path_buf();
        self.tmp_file = Some(tmp_file);
        self.config_with_path(content, &path)
    }

    #[track_caller]
    pub fn config_with_path(&mut self, content: &str, path: &Path) -> Result<&mut Self> {
        fs::write(path, format!("[env.\"{}\"]\n{}", &self.name, content))
            .wrap_err("Failed to write config to temporary file")?;
        self.config_file = path.to_path_buf();
        Ok(self)
    }

    #[track_caller]
    pub fn args(&mut self, args: Vec<&str>) -> Result<&mut Self> {
        self.args = args
            .into_iter()
            .map(|s| {
                match s {
                    "{name}" => self.name(),
                    "{config_path}" => self.config_path(),
                    _ => s,
                }
                .to_string()
            })
            .collect();
        Ok(self)
    }

    #[track_caller]
    pub fn envs(&mut self, envs: Vec<(&str, &str)>) -> Result<&mut Self> {
        self.envs.extend(
            envs.iter()
                .map(|s| (s.0.to_string(), s.1.to_string()))
                .collect::<Vec<(String, String)>>(),
        );
        Ok(self)
    }

    #[track_caller]
    pub fn working_dir(&mut self, working_dir: &str) -> Result<&mut Self> {
        let full_path = fs::canonicalize(working_dir)
            .wrap_err("Failed to create canonical path for working directory")?;
        if !full_path.exists() {
            return Err(eyre!("Provided working directory does not exist"));
        }
        if !full_path.is_dir() {
            return Err(eyre!("Provided working directory is not a directory"));
        }

        self.working_dir = Some(full_path);
        Ok(self)
    }

    #[track_caller]
    pub fn create_command(&mut self) -> Result<Command> {
        let bin_path = assert_cmd::cargo::cargo_bin(BINARY);
        let mut command = Command::new(&bin_path);

        if let Some(dir) = &self.working_dir {
            command.current_dir(dir);
        }
        command.env_clear();
        command.args(self.args.clone());
        command.envs(self.envs.clone());

        let command_vec: Vec<String> = std::iter::once(bin_path.display().to_string())
            .chain(self.args.clone())
            .collect();
        self.command_string = shell_words::join(command_vec);
        Ok(command)
    }
}

impl TestBase {
    pub fn config_path(&self) -> &str {
        self.config_file.to_str().unwrap()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn generate_random_environment_name() -> String {
        const LENGTH: usize = 32;
        let mut rng = thread_rng();

        // Environment containers already have a prefix
        // this extra one is to show its used in testing
        let first_chars: &str = "test-";

        // Characters for the rest of the positions [a-zA-Z0-9_.-]
        let other_chars: Vec<char> = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .chain(vec![b'_', b'.'])
            .map(char::from)
            .collect();

        let rest: String = (0..LENGTH)
            .map(|_| other_chars[rng.gen_range(0..other_chars.len())])
            .collect();

        format!("{}{}", first_chars, rest)
    }

    fn drop(&mut self) {
        if !self.name().is_empty() {
            let name_arg = format!("name=berth-{}", self.name());
            let containers = Command::new("docker")
                .args(["ps", "-a", "--filter", &name_arg, "--format", "{{.Names}}"])
                .output()
                .unwrap();
            let container = String::from_utf8(containers.stdout)
                .unwrap()
                .trim()
                .to_string();
            if !container.is_empty() {
                println!("Deleting container: {}", container);
                Command::new("docker")
                    .args(["rm", "-f", &container])
                    .output()
                    .unwrap();
            }
        }
    }
}

impl Drop for TestBase {
    fn drop(&mut self) {
        self.drop();
    }
}

pub struct TestHarness {
    base: TestBase,
}

pub struct RunningTestHarness {
    base: TestBase,
    session: PtySession,
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
    pub fn run(mut self, timeout_ms: Option<u64>) -> Result<RunningTestHarness> {
        let command = self.base.create_command()?;

        let session = spawn_command(command, timeout_ms).unwrap();
        Ok(RunningTestHarness {
            base: TestBase {
                config_file: mem::take(&mut self.base.config_file),
                tmp_file: self.base.tmp_file.take(),
                name: mem::take(&mut self.base.name),
                args: mem::take(&mut self.base.args),
                working_dir: mem::take(&mut self.base.working_dir),
                envs: mem::take(&mut self.base.envs),
                command_string: mem::take(&mut self.base.command_string),
            },
            session,
        })
    }
}

impl TestHarness {
    pub fn config_path(&self) -> &str {
        self.base.config_path()
    }

    pub fn name(&self) -> &str {
        self.base.name()
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
    pub fn expect_substring(mut self, expect: &str) -> Result<Self> {
        self.session
            .exp_any(vec![ReadUntil::String(expect.into())])
            .wrap_err("Failed to expect a substring")?;
        Ok(self)
    }

    #[track_caller]
    pub fn expect_terminate(mut self) -> Result<TerminatedTestHarness> {
        self.session.exp_eof().wrap_err("Failed to expect EOF")?;
        let wait_status = self
            .session
            .process
            .wait()
            .wrap_err("Failed to wait for process to exit")?;
        Ok(TerminatedTestHarness {
            base: TestBase {
                config_file: mem::take(&mut self.base.config_file),
                tmp_file: self.base.tmp_file.take(),
                name: mem::take(&mut self.base.name),
                args: mem::take(&mut self.base.args),
                working_dir: mem::take(&mut self.base.working_dir),
                envs: mem::take(&mut self.base.envs),
                command_string: mem::take(&mut self.base.command_string),
            },
            wait_status,
        })
    }
}

impl RunningTestHarness {
    pub fn config_path(&self) -> &str {
        self.base.config_path()
    }

    pub fn name(&self) -> &str {
        self.base.name()
    }
}

impl TerminatedTestHarness {
    #[track_caller]
    pub fn success(self) -> Result<()> {
        match self.wait_status {
            WaitStatus::Exited(_, 0) => Ok(()),
            WaitStatus::Exited(_, n) => Err(eyre!("Unexpected exit code: {}", n)),
            v => Err(eyre!("Unexpected Process WaitStatus {:?}", v)),
        }
    }

    #[track_caller]
    pub fn failure(self, expected_code: i32) -> Result<()> {
        match self.wait_status {
            WaitStatus::Exited(_, 0) => Err(eyre!("Unexpected successful exit")),
            WaitStatus::Exited(_, n) if n == expected_code => Ok(()),
            WaitStatus::Exited(_, n) => Err(eyre!("Unexpected exit code: {}", n)),
            v => Err(eyre!("Unexpected Process WaitStatus {:?}", v)),
        }
    }
}

impl TerminatedTestHarness {
    pub fn config_path(&self) -> &str {
        self.base.config_path()
    }

    pub fn name(&self) -> &str {
        self.base.name()
    }
}

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
        if !args.contains(&"--no-tty") {
            return Err(eyre!(
                "Error: TestOutput doesn't not provided a TTY, tests must use '--no-tty"
            ));
        }
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
        Ok(self)
    }

    #[track_caller]
    pub fn stderr(mut self, content: impl Into<String>) -> Result<Self> {
        self.stderr = content.into();
        Ok(self)
    }

    #[track_caller]
    pub fn code(mut self, code: i32) -> Result<Self> {
        self.exit_code = code;
        Ok(self)
    }

    #[track_caller]
    pub fn run(mut self) -> Result<()> {
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
