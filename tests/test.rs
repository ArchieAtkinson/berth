use rand::{thread_rng, Rng};
use rexpect::{
    process::wait::WaitStatus,
    session::{spawn_command, PtySession},
};
use std::{
    env, fs,
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

pub struct Test {
    config_file: PathBuf,
    tmp_file: Option<NamedTempFile>,
    name: String,
    process: Option<PtySession>,
    args: Vec<String>,
    working_dir: Option<String>,
    envs: Vec<(String, String)>,
}

impl Test {
    pub fn new() -> Self {
        Self {
            config_file: PathBuf::new(),
            tmp_file: None,
            name: Self::generate_random_environment_name(),
            process: None,
            args: Vec::new(),
            working_dir: None,
            envs: Vec::new(),
        }
    }

    pub fn config(&mut self, content: &str) -> &mut Self {
        self.tmp_file = Some(NamedTempFile::new().unwrap());
        let path = self.tmp_file.as_ref().unwrap().path().to_owned();
        self.config_with_path(content, path.as_path());
        self
    }

    pub fn config_with_path(&mut self, content: &str, path: &Path) -> &mut Self {
        self.config_file.push(path);
        fs::write(
            &self.config_file,
            format!("[env.\"{}\"]\n{}", self.name, content),
        )
        .unwrap();
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

    pub fn run(&mut self, timeout_ms: Option<u64>) -> &mut Self {
        let bin_path = assert_cmd::cargo::cargo_bin(BINARY);
        let mut command = Command::new(bin_path);

        if !self.get_args().is_empty() {
            command.args(self.get_args());
        }

        if self.working_dir.is_some() {
            command.current_dir(self.working_dir.clone().unwrap());
        }

        if !self.envs.is_empty() {
            command.envs(self.envs.clone());
        }

        self.process = Some(spawn_command(command, timeout_ms).unwrap());
        self
    }

    pub fn send_line(&mut self, cmd: &str) -> &mut Self {
        self.process.as_mut().unwrap().send_line(cmd).unwrap();
        self
    }

    pub fn expect_substring(&mut self, expect: &str) -> &mut Self {
        self.process
            .as_mut()
            .unwrap()
            .exp_regex(format!(".*?{}.*?", regex::escape(expect)).as_str())
            .unwrap();
        self
    }

    pub fn expect_terminate(&mut self) -> &mut Self {
        self.process.as_mut().unwrap().exp_eof().unwrap();
        self
    }

    pub fn success(&mut self) {
        match self.process.as_mut().unwrap().process.wait().unwrap() {
            WaitStatus::Exited(_, 0) => (),
            WaitStatus::Exited(_, n) => panic!("Unexpected exit code: {}", n),
            v => panic!("Unexpected Process WaitStatus {:?}", v),
        }
    }

    pub fn failure(&mut self, expected_code: i32) {
        match self.process.as_mut().unwrap().process.wait().unwrap() {
            WaitStatus::Exited(_, 0) => panic!("Unexpected sucessful exit"),
            WaitStatus::Exited(_, n) if n == expected_code => (),
            WaitStatus::Exited(_, n) => panic!("Unexpected exit code: {}", n),
            v => panic!("Unexpected Process WaitStatus {:?}", v),
        }
    }
}

impl Test {
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

    fn config_path(&self) -> &str {
        self.config_file.to_str().unwrap()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_args(&self) -> Vec<&str> {
        self.args.iter().map(|s| s.as_str()).collect()
    }
}
