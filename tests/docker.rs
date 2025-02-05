use color_eyre::Result;
use indoc::formatdoc;
use std::{fs::File, io::Write, process::Command};
use tempfile::TempDir;
use test::APK_ADD_ARGS;

pub mod test;
use crate::test::TestHarness;

#[test]
fn mount() -> Result<()> {
    let tmp_dir = TempDir::new().unwrap();

    let mounted_file_name = "test.txt";
    let file_path = tmp_dir.path().join(mounted_file_name);

    let mut tmp_file = File::create(file_path).unwrap();
    let file_text = "Hello World";
    writeln!(tmp_file, "{}", file_text).unwrap();

    let container_mount_dir = "/home/mount";

    TestHarness::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            mounts = ["{}:{}"]    
            "#,
            tmp_dir.path().to_str().unwrap(),
            container_mount_dir
        ))?
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(5000)?
        .send_line(&format!("cat {container_mount_dir}/{mounted_file_name}"))?
        .expect_string(&format!("{file_text}"))?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    tmp_dir.close().unwrap();
    Ok(())
}

#[test]
fn exec_cmds() -> Result<()> {
    TestHarness::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            exec_cmds = ["apk add {} asciiquarium"]
            init_cmd = "/bin/ash"    
            "#,
            APK_ADD_ARGS
        ))?
        .args(vec![
            "--cleanup",
            "--config-path",
            "[config_path]",
            "[name]",
        ])?
        .run(5000)?
        .send_line("which asciiquarium")?
        .expect_string("/usr/bin/asciiquarium")?
        .send_line("exit")?
        .expect_terminate()?
        .success()
}

#[test]
fn mount_working_dir() -> Result<()> {
    let tmp_dir = TempDir::new().unwrap();

    let mounted_file_name = "test.txt";
    let file_path = tmp_dir.path().join(mounted_file_name);

    let mut tmp_file = File::create(file_path).unwrap();
    let file_text = "Hello World";
    writeln!(tmp_file, "{}", file_text).unwrap();

    let container_mount_dir = "/berth";

    TestHarness::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            mounts = ["$PWD:{0}"]
            entry_dir = "{0}"
            "#,
            container_mount_dir,
        ))?
        .envs(vec![("PWD", tmp_dir.path().to_str().unwrap())])?
        .args(vec![
            "--cleanup",
            "--config-path",
            "[config_path]",
            "[name]",
        ])?
        .run(5000)?
        .send_line(&format!("cat {mounted_file_name}"))?
        .expect_string(&format!("{file_text}"))?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    tmp_dir.close().unwrap();
    Ok(())
}

#[test]
fn keep_container_running_if_one_terminal_exits() -> Result<()> {
    let is_container_running = |name: &str| {
        let name_filter = format!("name={}", name);
        let mut ls_cmd = Command::new("docker");
        ls_cmd.args([
            "container",
            "ls",
            "--format",
            "{{.Names}}",
            "--filter",
            &name_filter,
        ]);

        let running_containers = String::from_utf8(ls_cmd.output().unwrap().stdout).unwrap();
        running_containers.contains(name)
    };

    let harness = TestHarness::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            "#,
        ))?
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(5000)?
        .send_line("echo $0")?;

    let container_name = harness.name().to_string();

    // As we don't expect any value in harness, the container won't
    // have started if we don't sleep before checking
    std::thread::sleep(std::time::Duration::from_millis(2000));
    assert!(is_container_running(&container_name));

    TestHarness::new()
        .args(vec![
            "--config-path",
            harness.config_path(),
            &container_name,
        ])?
        .run(5000)?
        .send_line("echo $0")?
        .expect_string("/bin/ash")?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    assert!(is_container_running(&container_name));

    harness
        .send_line("echo $0")?
        .expect_string("/bin/ash")?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    assert!(!is_container_running(&container_name));
    Ok(())
}
