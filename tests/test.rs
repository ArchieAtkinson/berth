use rand::{thread_rng, Rng};
use rexpect::{
    process::wait::WaitStatus,
    session::{spawn_command, PtySession},
    ReadUntil,
};
use std::{
    env, fs,
    marker::PhantomData,
    mem,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;

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
    working_dir: Option<String>,
    envs: Vec<(String, String)>,
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
        }
    }

    pub fn config(&mut self, content: &str) -> &mut Self {
        self.tmp_file = Some(NamedTempFile::new().unwrap());
        let path = self.tmp_file.as_ref().unwrap().path().to_path_buf();
        self.config_with_path(content, &path);
        self
    }

    pub fn config_with_path(&mut self, content: &str, path: &Path) -> &mut Self {
        fs::write(path, format!("[env.\"{}\"]\n{}", &self.name, content)).unwrap();
        self.config_file = path.to_path_buf();
        self
    }

    pub fn args(&mut self, args: Vec<&str>) -> &mut Self {
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
        self
    }

    pub fn envs(&mut self, envs: Vec<(&str, &str)>) -> &mut Self {
        self.envs.extend(
            envs.iter()
                .map(|s| (s.0.to_string(), s.1.to_string()))
                .collect::<Vec<(String, String)>>(),
        );
        self
    }

    pub fn working_dir(&mut self, working_dir: &str) -> &mut Self {
        self.working_dir = Some(working_dir.to_string());
        self
    }

    pub fn create_command(&mut self) -> Command {
        let bin_path = assert_cmd::cargo::cargo_bin(BINARY);
        let mut command = Command::new(bin_path);

        if let Some(dir) = &self.working_dir {
            command.current_dir(dir);
        }
        command.env_clear();
        command.args(self.args.clone());
        command.envs(self.envs.clone());
        command
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

pub struct Configuring;
pub struct Running;
pub struct Terminated;

pub struct TestHarness<S> {
    base: TestBase,
    session: Option<PtySession>,
    _state: PhantomData<S>,
}

impl TestHarness<Configuring> {
    pub fn new() -> Self {
        Self {
            base: TestBase::new(),
            session: None,
            _state: PhantomData,
        }
    }

    pub fn config(mut self, content: &str) -> Self {
        self.base.config(content);
        self
    }

    pub fn config_with_path(mut self, content: &str, path: &Path) -> Self {
        self.base.config_with_path(content, path);
        self
    }

    pub fn args(mut self, args: Vec<&str>) -> Self {
        self.base.args(args);
        self
    }

    pub fn envs(mut self, envs: Vec<(&str, &str)>) -> Self {
        self.base.envs(envs);
        self
    }

    pub fn working_dir(mut self, working_dir: &str) -> Self {
        self.base.working_dir(working_dir);
        self
    }

    pub fn run(mut self, timeout_ms: Option<u64>) -> TestHarness<Running> {
        let command = self.base.create_command();

        let session = spawn_command(command, timeout_ms).unwrap();
        TestHarness {
            base: TestBase {
                config_file: mem::take(&mut self.base.config_file),
                tmp_file: self.base.tmp_file.take(),
                name: mem::take(&mut self.base.name),
                args: mem::take(&mut self.base.args),
                working_dir: mem::take(&mut self.base.working_dir),
                envs: mem::take(&mut self.base.envs),
            },
            session: Some(session),
            _state: PhantomData,
        }
    }
}

impl TestHarness<Running> {
    pub fn send_line(mut self, cmd: &str) -> Self {
        self.session.as_mut().unwrap().send_line(cmd).unwrap();
        self
    }

    pub fn expect_substring(mut self, expect: &str) -> Self {
        self.session
            .as_mut()
            .unwrap()
            .exp_any(vec![ReadUntil::String(expect.into())])
            .unwrap();
        self
    }

    pub fn expect_terminate(mut self) -> TestHarness<Terminated> {
        self.session.as_mut().unwrap().exp_eof().unwrap();
        TestHarness {
            base: TestBase {
                config_file: mem::take(&mut self.base.config_file),
                tmp_file: self.base.tmp_file.take(),
                name: mem::take(&mut self.base.name),
                args: mem::take(&mut self.base.args),
                working_dir: mem::take(&mut self.base.working_dir),
                envs: mem::take(&mut self.base.envs),
            },
            session: mem::take(&mut self.session),
            _state: PhantomData,
        }
    }
}

impl TestHarness<Terminated> {
    pub fn success(mut self) {
        match self.session.as_mut().unwrap().process.wait().unwrap() {
            WaitStatus::Exited(_, 0) => (),
            WaitStatus::Exited(_, n) => panic!("Unexpected exit code: {}", n),
            v => panic!("Unexpected Process WaitStatus {:?}", v),
        }
    }

    pub fn failure(mut self, expected_code: i32) {
        match self.session.as_mut().unwrap().process.wait().unwrap() {
            WaitStatus::Exited(_, 0) => panic!("Unexpected successful exit"),
            WaitStatus::Exited(_, n) if n == expected_code => (),
            WaitStatus::Exited(_, n) => panic!("Unexpected exit code: {}", n),
            v => panic!("Unexpected Process WaitStatus {:?}", v),
        }
    }
}

impl<S> TestHarness<S> {
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

    pub fn config(mut self, content: &str) -> Self {
        self.base.config(content);
        self
    }

    pub fn config_with_path(mut self, content: &str, path: &Path) -> Self {
        self.base.config_with_path(content, path);
        self
    }

    pub fn args(mut self, args: Vec<&str>) -> Self {
        if !args.contains(&"--no-tty") {
            panic!("Error: TestOutput doesn't not provided a TTY, tests must use '--no-tty");
        }
        self.base.args(args);
        self
    }

    pub fn envs(mut self, envs: Vec<(&str, &str)>) -> Self {
        self.base.envs(envs);
        self
    }

    pub fn working_dir(mut self, working_dir: &str) -> Self {
        self.base.working_dir(working_dir);
        self
    }

    pub fn stdout(mut self, content: impl Into<String>) -> Self {
        self.stdout = content.into();
        self
    }

    pub fn stderr(mut self, content: impl Into<String>) -> Self {
        self.stderr = content.into();
        self
    }

    pub fn code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }

    pub fn run(mut self) {
        let output = self.base.create_command().output().unwrap();

        let output_stdout = String::from_utf8(output.stdout).unwrap();
        let output_stderr = String::from_utf8(output.stderr).unwrap();
        let output_exit_code = output.status.code().unwrap();

        assert_eq!(output_stdout, self.stdout);
        assert_eq!(output_stderr, self.stderr);
        assert_eq!(output_exit_code, self.exit_code);
    }
}
