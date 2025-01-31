use std::{
    fs::{self},
    path::PathBuf,
};

use berth::cli::AppConfig;
use indoc::indoc;
use tempfile::{NamedTempFile, TempDir};
use test::TestHarness;

pub mod test;

#[test]
fn no_commands() {
    let args = vec!["berth"];

    let app_config = AppConfig::new(&args);
    assert!(app_config.is_err());

    let err = app_config.err().unwrap();
    assert!(err
        .to_string()
        .contains("the following required arguments were not provided"))
}

#[test]
fn env_name_with_config_in_xdg_config_path() {
    let tmp_dir = TempDir::new().unwrap();
    let file_path = tmp_dir
        .path()
        .join(".config")
        .join("berth")
        .join("config.toml");
    fs::create_dir_all(&file_path.parent().unwrap()).unwrap();

    TestHarness::new()
        .config_with_path(
            &indoc!(
                r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            "#,
            ),
            &file_path,
        )
        .args(vec!["--cleanup", "{name}"])
        .envs(vec![("XDG_CONFIG_PATH", tmp_dir.path().to_str().unwrap())])
        .run(Some(5000))
        .expect_substring("/ #")
        .send_line("exit")
        .expect_terminate()
        .success();

    tmp_dir.close().unwrap();
}

#[test]
fn env_name_with_config_in_home_path() {
    let tmp_dir = TempDir::new().unwrap();
    let file_path = tmp_dir
        .path()
        .join(".config")
        .join("berth")
        .join("config.toml");
    fs::create_dir_all(&file_path.parent().unwrap()).unwrap();

    TestHarness::new()
        .config_with_path(
            &indoc!(
                r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            "#,
            ),
            &file_path,
        )
        .args(vec!["--cleanup", "{name}"])
        .envs(vec![("HOME", tmp_dir.path().to_str().unwrap())])
        .run(Some(5000))
        .expect_substring("/ #")
        .send_line("exit")
        .expect_terminate()
        .success();

    tmp_dir.close().unwrap();
}

#[test]
fn env_name_with_no_config_in_env() {
    // let args = vec!["berth", "Name"];

    // let empty_env_var = AppEnvVar::new()
    //     .set_var("HOME", "")
    //     .set_var("XDG_CONFIG_PATH", "");
    // let app_config = AppConfig::new(args, &empty_env_var).err().unwrap();
    // assert_eq!(
    //     app_config.to_string(),
    //     "Could not find config file in $XDG_CONFIG_PATH or $HOME"
    // );

    TestHarness::new()
        .config(&indoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            "#,
        ))
        .args(vec!["--cleanup", "{name}"])
        .envs(vec![("HOME", ""), ("XDG_CONFIG_PATH", "")])
        .run(Some(5000))
        .expect_substring("Could not find config file in $XDG_CONFIG_PATH or $HOME")
        .expect_terminate()
        .failure(1);
}

#[test]
fn valid_config_file() {
    let config_file = NamedTempFile::new().unwrap();
    let config_file_path = config_file.path().to_str().unwrap();
    let args = vec!["berth", "--config-path", config_file_path, "Name"];

    let app_config = AppConfig::new(args).unwrap();
    assert_eq!(app_config.env_name, "Name");
    assert_eq!(app_config.config_path.to_str(), Some(config_file_path))
}

#[test]
fn nonexistent_config_file() {
    let not_real_file = PathBuf::from(" ");
    let not_real_file_path = not_real_file.as_path().to_str().unwrap();
    let args = vec!["berth", "--config-path", not_real_file_path, "Name"];

    let app_config = AppConfig::new(args).err().unwrap();
    let expected_error_text = format!(
        "Could not find file at 'config-path': {:?}",
        not_real_file_path
    );
    assert_eq!(app_config.to_string(), expected_error_text);
}

#[test]
fn incorrect_option_command() {
    let args = vec!["berth", "--bad-command"];

    let app_config = AppConfig::new(args).err().unwrap();
    assert_eq!(
        app_config.to_string(),
        indoc!(
            r#"
        error: unexpected argument '--bad-command' found

          tip: to pass '--bad-command' as a value, use '-- --bad-command'

        Usage: berth [OPTIONS] <ENV_NAME>

        For more information, try '--help'.
        "#
        )
    );
}
