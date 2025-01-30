use indoc::{formatdoc, indoc};
use std::{fs::File, io::Write, process::Command};
use tempfile::TempDir;

pub mod test;
use crate::test::Test;

#[test]
fn mount() {
    let tmp_dir = TempDir::new().unwrap();

    let mounted_file_name = "test.txt";
    let file_path = tmp_dir.path().join(mounted_file_name);

    let mut tmp_file = File::create(file_path).unwrap();
    let file_text = "Hello World";
    writeln!(tmp_file, "{}", file_text).unwrap();

    let container_mount_dir = "/home/mount";

    Test::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            mounts = ["{}:{}"]    
            "#,
            tmp_dir.path().to_str().unwrap(),
            container_mount_dir
        ))
        .args(vec![
            "--cleanup",
            "--config-path",
            "{config_path}",
            "{name}",
        ])
        .run(Some(5000))
        .expect_substring("/ #")
        .send_line(&format!(
            "cat {}/{}",
            container_mount_dir, mounted_file_name
        ))
        .expect_substring(file_text)
        .send_line("exit")
        .expect_terminate()
        .success();

    tmp_dir.close().unwrap();
}

#[test]
fn exec_cmds() {
    Test::new()
        .config(indoc!(
            r#"
            image = "alpine:edge"
            exec_cmds = ["apk add helix"]
            init_cmd = "/bin/ash"    
            "#
        ))
        .args(vec![
            "--cleanup",
            "--config-path",
            "{config_path}",
            "{name}",
        ])
        .run(Some(5000))
        .expect_substring("/ #")
        .send_line("which hx")
        .expect_substring("/usr/bin/hx")
        .send_line("exit")
        .expect_terminate()
        .success();
}

#[test]
fn mount_working_dir() {
    let tmp_dir = TempDir::new().unwrap();

    let mounted_file_name = "test.txt";
    let file_path = tmp_dir.path().join(mounted_file_name);

    let mut tmp_file = File::create(file_path).unwrap();
    let file_text = "Hello World";
    writeln!(tmp_file, "{}", file_text).unwrap();

    let container_mount_dir = "/berth";

    Test::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            mounts = ["$PWD:{0}"]
            entry_dir = "{0}"
            "#,
            container_mount_dir,
        ))
        .envs(vec![("PWD", tmp_dir.path().to_str().unwrap())])
        .args(vec![
            "--cleanup",
            "--config-path",
            "{config_path}",
            "{name}",
        ])
        .run(Some(2500))
        .expect_substring(&format!("{} #", container_mount_dir))
        .send_line("pwd && ls")
        .send_line(&format!("cat {}", mounted_file_name))
        .expect_substring(file_text)
        .send_line("exit")
        .expect_terminate()
        .success();

    tmp_dir.close().unwrap();
}

#[test]
fn keep_container_running_if_one_terminal_exits() {
    let mut t1 = Test::new();
    t1.config(&formatdoc!(
        r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            "#,
    ))
    .args(vec!["--config-path", "{config_path}", "{name}"])
    .run(Some(2500))
    .expect_substring("/ #");

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

    assert!(is_container_running(&t1.name()));

    Test::new()
        .args(vec!["--config-path", t1.config_path(), t1.name()])
        .run(Some(2500))
        .expect_substring("/ #")
        .send_line("exit")
        .expect_terminate()
        .success();

    assert!(is_container_running(&t1.name()));

    t1.send_line("exit").expect_terminate().success();

    assert!(!is_container_running(&t1.name()));
}
