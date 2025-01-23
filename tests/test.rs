use indoc::formatdoc;
use rand::{thread_rng, Rng};
use rexpect::session::spawn_command;
use std::{fs::File, io::Write, path::PathBuf, process::Command};
use tempfile::{NamedTempFile, TempDir};

const BINARY: &str = env!("CARGO_PKG_NAME");

pub struct Test {
    config_file: PathBuf,
}

impl Test {
    pub fn new() -> Self {
        Self {
            config_file: PathBuf::new(),
        }
    }

    pub fn config(mut self, content: &str) -> Self {
        let random_container_name = Self::generate_random_container_name();
        let content = content.replace("{name}", &random_container_name);
        let mut config = NamedTempFile::new().unwrap();
        write!(config, "{}", content).unwrap();
        self.config_file = config.path().to_path_buf();
        self
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
}

fn basic_exec() {
    let tmp_dir = TempDir::new().unwrap();
    let mounted_file_name = "test.txt";
    let file_path = tmp_dir.path().join(mounted_file_name);
    let mut tmp_file = File::create(file_path).unwrap();
    writeln!(tmp_file, "Hello World").unwrap();

    let container_mount_dir = "/home/mount";

    let container_name = "bob";
    let content = formatdoc!(
        r#"
        [[env]]
        name = "{}"
        image = "alpine:edge"
        exec_cmds = ["apk", "add", "helix", "bash"]
        mounts = ["{}:{}"]
        "#,
        container_name,
        tmp_dir.path().to_str().unwrap(),
        container_mount_dir
    );

    let mut preset = NamedTempFile::new().unwrap();
    preset.write(content.as_bytes()).unwrap();

    let bin_path = assert_cmd::cargo::cargo_bin(BINARY);

    let run_result = (|| -> Result<(), rexpect::error::Error> {
        let mut test_command = Command::new(bin_path);
        test_command.args([
            "--config-file",
            preset.path().to_str().unwrap(),
            &container_name,
        ]);

        let mut process = spawn_command(test_command, Some(50000))?;
        process.send_line("clear")?;

        process.send_line("which hx")?;
        process.exp_regex(".*?/usr/bin/hx.*?")?;

        process.send_line(&format!(
            "cat {}/{}",
            container_mount_dir, mounted_file_name
        ))?;
        process.exp_regex(".*?Hello World.*?")?;

        process.send_line("exit")?;
        process.exp_eof()?;

        Ok(())
    })();

    let cleanup_status = Command::new("docker")
        .args(["rm", "-f", &container_name.to_string()])
        .status();

    preset.close().unwrap();
    tmp_dir.close().unwrap();
    run_result.expect("Failed");
    cleanup_status.expect("Failed to clean up");
}
