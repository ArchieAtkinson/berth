use bollard::{container::ListContainersOptions, Docker};
use color_eyre::Result;
use indoc::{formatdoc, indoc};
use serial_test::serial;
use std::{collections::HashMap, fs::File, io::Write};
use tempfile::{NamedTempFile, TempDir};
use test_utils::{TestHarness, TestOutput, APK_ADD_ARGS, DEFAULT_TIMEOUT};

pub mod test_utils;

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
#[serial]
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
        .run(DEFAULT_TIMEOUT)?
        .send_line(&format!("cat {container_mount_dir}/{mounted_file_name}"))?
        .expect_string(file_text)?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    tmp_dir.close().unwrap();
    Ok(())
}

#[test]
#[serial]
fn copy_cmds() -> Result<()> {
    let mut tmp_file = NamedTempFile::new().unwrap();
    let tmp_file_path = tmp_file.path().to_str().unwrap().to_string();
    let file_text = "Hello World";
    writeln!(tmp_file, "{}", file_text).unwrap();

    TestHarness::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            entry_cmd = "/bin/ash"
            cp_cmds = ["{} CONTAINER:{}"]
            create_options = ["-it"]
            entry_options = ["-it"]
            "#,
            tmp_file_path,
            tmp_file_path
        ))?
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(DEFAULT_TIMEOUT)?
        .send_line(&format!("cat {}", tmp_file_path))?
        .expect_string(file_text)?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    tmp_file.close().unwrap();
    Ok(())
}

#[test]
#[serial]
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
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(DEFAULT_TIMEOUT)?
        .send_line("which asciiquarium")?
        .expect_string("/usr/bin/asciiquarium")?
        .send_line("exit")?
        .expect_terminate()?
        .success()
}

#[tokio::test]
async fn build() -> Result<()> {
    let mut test = TestOutput::new()
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
        .args(vec!["--config-path", "[config_path]", "--build", "[name]"])?
        .stderr("Using config file at \"[config_path]\"\n")?
        .code(0)?;

    test.run()?;

    std::thread::sleep(std::time::Duration::from_secs(1)); // wait for container to stop

    let docker = Docker::connect_with_local_defaults().unwrap();
    assert!(!is_container_running(&docker, test.name()).await);
    Ok(())
}

#[test]
#[serial]
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
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(DEFAULT_TIMEOUT)?
        .send_line(&format!("cat {mounted_file_name}"))?
        .expect_string(file_text)?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    tmp_dir.close().unwrap();
    Ok(())
}

#[tokio::test]
#[serial]
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
        .run(DEFAULT_TIMEOUT)?
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
        .run(DEFAULT_TIMEOUT)?
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

#[test]
#[serial]
fn dockerfile() -> Result<()> {
    let dockerfile = NamedTempFile::new().unwrap();
    let content = indoc! {
    r#"
    FROM alpine:edge
    RUN apk add asciiquarium
    "#};
    write!(&dockerfile, "{}", content).unwrap();

    TestHarness::new()
        .config(&formatdoc!(
            r#"
            dockerfile = "{}"
            entry_cmd = "/bin/ash"
            create_options = ["-it"]
            entry_options = ["-it"]
            "#,
            dockerfile.path().to_str().unwrap(),
        ))?
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(DEFAULT_TIMEOUT)?
        .send_line("which asciiquarium")?
        .expect_string("/usr/bin/asciiquarium")?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    dockerfile.close().unwrap();
    Ok(())
}

#[test]
fn badly_formed_dockerfile() -> Result<()> {
    let dockerfile = NamedTempFile::new().unwrap();
    let content = indoc! {
    r#"
    FRO alpine:edge
    RUN apk add asciiquarium
    "#};
    write!(&dockerfile, "{}", content).unwrap();

    TestHarness::new()
        .config(&formatdoc!(
            r#"
            dockerfile = "{}"
            entry_cmd = "/bin/ash"
            create_options = ["-it"]
            entry_options = ["-it"]
            "#,
            dockerfile.path().to_str().unwrap(),
        ))?
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(DEFAULT_TIMEOUT)?
        // Can't test full output as we don't know the image name
        .expect_string("Error: cli::container::command::exitcode")?
        .expect_string("The following command return an error code:")?
        .expect_string(indoc!(
            r#"help: #0 building with "default" instance using docker driver"#
        ))?
        .expect_terminate()?
        .failure(1)?;

    dockerfile.close().unwrap();
    Ok(())
}
