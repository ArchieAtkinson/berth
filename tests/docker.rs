use indoc::formatdoc;
use rand::{thread_rng, Rng};
use rexpect::session::spawn_command;
use std::{fs::File, io::Write, process::Command};
use tempfile::{NamedTempFile, TempDir};

const BINARY: &str = env!("CARGO_PKG_NAME");

// #[test]
// fn basic_exec() {
//     let content = indoc!(
//         r#"
//         [[env]]
//         name = "TestContainer"
//         image = "alpine:edge"
//         exec_cmds = ["apk", "add", "helix", "bash"]
//         "#
//     );

//     let mut preset = NamedTempFile::new().unwrap();
//     preset.write(content.as_bytes()).unwrap();

//     let mut cmd = Command::cargo_bin(BINARY).unwrap();
//     let assert = cmd
//         .args(["--config-file", preset.path().to_str().unwrap()])
//         .write_stdin("which hx\n")
//         .assert();
//     assert.success().stdout("/bin/usr/hx");

//     preset.close().unwrap();
// }

fn generate_random_docker_name() -> String {
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

#[test]
fn basic_exec() {
    let tmp_dir = TempDir::new().unwrap();
    let mounted_file_name = "test.txt";
    let file_path = tmp_dir.path().join(mounted_file_name);
    let mut tmp_file = File::create(file_path).unwrap();
    writeln!(tmp_file, "Hello World").unwrap();

    let container_mount_dir = "/home/mount";

    let container_name = generate_random_docker_name();
    let content = formatdoc!(
        r#"
        [[env]]
        name = "{}"
        image = "alpine:edge"
        exec_cmds = ["apk add helix bash"]
        mounts = ["{}:{}"]
        init_cmd = "/bin/ash"
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
            "--config-path",
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
