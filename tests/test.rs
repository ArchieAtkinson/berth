use indoc::indoc;
use rand::{thread_rng, Rng};
use rexpect::{
    process::wait::WaitStatus,
    session::{spawn_command, PtySession},
};
use std::{
    collections::HashMap,
    io::Write,
    process::{Command, Stdio},
};
use tempfile::NamedTempFile;

const BINARY: &str = env!("CARGO_PKG_NAME");

pub struct Test {
    config_file: NamedTempFile,
    name: String,
    process: Option<PtySession>,
}

impl Test {
    pub fn new() -> Self {
        Self {
            config_file: NamedTempFile::new().unwrap(),
            name: Self::generate_random_container_name(),
            process: None,
        }
    }

    pub fn env(&mut self, content: &str) -> &mut Self {
        write!(self.config_file, "[env.\"{}\"]\n{}", self.name, content).unwrap();
        self
    }

    pub fn run(&mut self, args: Vec<&str>, timeout_ms: Option<u64>) -> &mut Self {
        let bin_path = assert_cmd::cargo::cargo_bin(BINARY);
        let mut command = Command::new(bin_path);

        let replacements: HashMap<&str, &str> = HashMap::from([
            ("{name}", self.name()),
            ("{config_path}", self.config_path()),
        ]);

        let result: Vec<&str> = args
            .iter()
            .copied()
            .map(|s| *replacements.get(s).unwrap_or(&s))
            .collect();

        command.args(result);
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
            .exp_regex(format!(".*?{}.*?", expect).as_str())
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
    fn generate_random_container_name() -> String {
        const LENGTH: usize = 63;
        let mut rng = thread_rng();

        let first_chars: &str = "dh-test-";

        // Characters for the rest of the positions [a-zA-Z0-9_.-]
        let other_chars: Vec<char> = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .chain(vec![b'_', b'.', b'-'])
            .map(char::from)
            .collect();

        let rest: String = (0..LENGTH - first_chars.len())
            .map(|_| other_chars[rng.gen_range(0..other_chars.len())])
            .collect();

        format!("{}{}", first_chars, rest)
    }

    fn config_path(&self) -> &str {
        self.config_file.path().to_str().unwrap()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for Test {
    fn drop(&mut self) {
        Command::new("docker")
            .args(["rm", "-f", self.name()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok();
    }
}

#[test]
fn simple_int() {
    Test::new()
        .env(indoc!(
            r#"
            image = "alpine:edge"
            exec_cmds = ["apk add helix"]
            init_cmd = "/bin/ash"    
            "#
        ))
        .run(vec!["--config-path", "{config_path}", "{name}"], Some(5000))
        .expect_substring("/ #")
        .send_line("which hx")
        .expect_substring("/usr/bin/hx")
        .send_line("exit")
        .expect_terminate()
        .success();
}

#[test]
fn incorrect_arg() {
    Test::new()
        .env("")
        .run(vec!["--config", "{config_path}"], Some(5000))
        .expect_substring("error: unexpected argument '--config' found")
        .expect_terminate()
        .failure(1);
}
