use bollard::{container::ListContainersOptions, Docker};
use color_eyre::Result;
use indoc::{formatdoc, indoc};
use serial_test::serial;
use std::{
    collections::HashMap,
    fs::{create_dir, File},
    io::Write,
};
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
fn relative_to_config_file() -> Result<()> {
    let mut copy_file = NamedTempFile::new().unwrap();
    let file_text = "Hello World";
    writeln!(copy_file, "{}", file_text).unwrap();
    let copy_file_path = copy_file.path();

    let config_file = NamedTempFile::new().unwrap();
    let config_file_path = config_file.path();

    let harness = TestHarness::new().config_with_path(
        &formatdoc!(
            r#"
            image = "alpine:edge"
            entry_cmd = "/bin/ash"
            cp_cmds = ["{} CONTAINER:{}"]
            create_options = ["-it"]
            entry_options = ["-it"]
            "#,
            copy_file_path.file_name().unwrap().to_str().unwrap(),
            copy_file_path.to_str().unwrap()
        ),
        &config_file_path,
    )?;

    harness
        .args(vec![
            "--config-path",
            config_file_path.to_str().unwrap(),
            "[name]",
        ])?
        .run(DEFAULT_TIMEOUT)?
        .send_line(&format!("cat {}", copy_file_path.to_str().unwrap()))?
        .expect_string(file_text)?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    copy_file.close().unwrap();
    config_file.close().unwrap();
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
#[serial]
fn dockerfile_default_build_context() -> Result<()> {
    let dir = TempDir::new().unwrap();
    let dockerfile = File::create(dir.path().join("dockerfile")).unwrap();
    File::create(dir.path().join("test_file")).unwrap();
    let content = indoc! {
    r#"
    FROM alpine:edge
    COPY test_file test_file
    "#};
    write!(&dockerfile, "{}", content).unwrap();

    TestHarness::new()
        .config_with_path(
            &formatdoc!(
                r#"
            dockerfile = "{}"
            entry_cmd = "/bin/ash"
            create_options = ["-it"]
            entry_options = ["-it"]
            "#,
                dir.path().join("dockerfile").to_str().unwrap(),
            ),
            &dir.path().join("config.toml"),
        )?
        .args(vec![
            "--config-path",
            dir.path().join("config.toml").to_str().unwrap(),
            "[name]",
        ])?
        .run(DEFAULT_TIMEOUT)?
        .send_line("ls")?
        .expect_string("test_file")?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    dir.close()?;
    Ok(())
}

#[test]
#[serial]
fn dockerfile_provided_build_context() -> Result<()> {
    let dir = TempDir::new().unwrap();
    create_dir(dir.path().join("dockerfile_dir")).unwrap();
    create_dir(dir.path().join("context_dir")).unwrap();

    let dockerfile = File::create(dir.path().join("dockerfile_dir/dockerfile")).unwrap();
    File::create(dir.path().join("context_dir/test_file")).unwrap();
    let content = indoc! {
    r#"
    FROM alpine:edge
    COPY test_file test_file
    "#};
    write!(&dockerfile, "{}", content).unwrap();

    TestHarness::new()
        .config(&formatdoc!(
            r#"
            dockerfile = "{}"
            build_context = "{}"
            entry_cmd = "/bin/ash"
            create_options = ["-it"]
            entry_options = ["-it"]
            "#,
            dir.path()
                .join("dockerfile_dir/dockerfile")
                .to_str()
                .unwrap(),
            dir.path().join("context_dir").to_str().unwrap(),
        ))?
        .args(vec!["--config-path", "[config_path]", "[name]"])?
        .run(DEFAULT_TIMEOUT)?
        .send_line("ls")?
        .expect_string("test_file")?
        .send_line("exit")?
        .expect_terminate()?
        .success()?;

    dir.close()?;
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

    TestOutput::new()
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
        .stderr("Error: cli::container::command::exitcode")?
        .code(1)?;

    dockerfile.close().unwrap();
    Ok(())
}
