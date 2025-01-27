use indoc::{formatdoc, indoc};
use std::{fs::File, io::Write};
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

    let container_mount_dir = format!(
        "/berth/{}",
        tmp_dir.path().file_name().unwrap().to_str().unwrap()
    );

    Test::new()
        .config(&formatdoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            mount_working_dir = true
            "#,
        ))
        .working_dir(tmp_dir.path().to_str().unwrap())
        .args(vec![
            "--cleanup",
            "--config-path",
            "{config_path}",
            "{name}",
        ])
        .run(Some(5000))
        .expect_substring(&format!("{} #", container_mount_dir))
        .send_line(&format!("cat {}", mounted_file_name))
        .expect_substring(file_text)
        .send_line("exit")
        .expect_terminate()
        .success();

    tmp_dir.close().unwrap();
}
