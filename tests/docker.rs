use bollard::{container::ListContainersOptions, Docker};
use color_eyre::Result;
use indoc::formatdoc;
use std::{collections::HashMap, fs::File, io::Write};
use tempfile::TempDir;
use test::APK_ADD_ARGS;

pub mod test;
use crate::test::TestHarness;

async fn is_container_running(docker: &Docker, name: &str) -> bool {
    let mut filters = HashMap::new();
    filters.insert("name", vec![name]);
    let options = Some(ListContainersOptions {
        filters,
        ..Default::default()
    });
    !docker.list_containers(options).await.unwrap().is_empty()
}

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
            entry_cmd = "/bin/ash"
            create_options = ["-it", "-v{}:{}"]
            entry_options = ["-it"]
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
            entry_cmd = "/bin/ash"
            create_options = ["-it"]
            entry_options = ["-it"]    
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
            entry_cmd = "/bin/ash"
            create_options = ["-it", "-v $PWD:{0}"]
            entry_options = ["-it", "-w {0}"]
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

#[tokio::test]
async fn keep_container_running_if_one_terminal_exits() -> Result<()> {
    let docker = Docker::connect_with_local_defaults().unwrap();

    let harness = TestHarness::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            entry_cmd = "/bin/ash"
            create_options = ["-it"]
            entry_options = ["-it"]
            "#,
        ))?
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(5000)?
        .send_line("echo $0")?
        .expect_string("/bin/ash")?;

    let container_name = harness.name().to_string();

    assert!(is_container_running(&docker, &container_name).await);

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

    assert!(is_container_running(&docker, &container_name).await);

    harness
        .send_line("echo $0")?
        .expect_string("/bin/ash")?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    assert!(!is_container_running(&docker, &container_name).await);
    Ok(())
}
