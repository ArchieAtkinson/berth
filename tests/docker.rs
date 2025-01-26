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
        .env(&formatdoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            mounts = ["{}:{}"]    
            "#,
            tmp_dir.path().to_str().unwrap(),
            container_mount_dir
        ))
        .run(vec!["--config-path", "{config_path}", "{name}"], Some(5000))
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
