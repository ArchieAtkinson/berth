use berth::cli::AppConfig;
use color_eyre::Result;
use indoc::{formatdoc, indoc};
use pretty_assertions::assert_eq;
use std::{
    fs::{self},
    path::PathBuf,
};
use tempfile::{NamedTempFile, TempDir};
use test::{TestHarness, TestOutput};

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
fn env_name_with_config_in_xdg_config_path() -> Result<()> {
    let tmp_dir = TempDir::new().unwrap();
    let file_path = tmp_dir
        .path()
        .join(".config")
        .join("berth")
        .join("config.toml");
    fs::create_dir_all(&file_path.parent().unwrap()).unwrap();

    TestOutput::new()
        .config_with_path(
            &indoc!(
                r#"
            image = "alpine:edge"
            init_cmd = "true"
            "#,
            ),
            &file_path,
        )?
        .args(vec!["--no-tty", "[name]"])?
        .envs(vec![("XDG_CONFIG_PATH", tmp_dir.path().to_str().unwrap())])?
        .stderr(format!("Using config file at {:?}\n", file_path))?
        .code(0)?
        .run()?;

    tmp_dir.close().unwrap();

    Ok(())
}

#[test]
fn env_name_with_config_in_home_path() -> Result<()> {
    let tmp_dir = TempDir::new().unwrap();
    let file_path = tmp_dir
        .path()
        .join(".config")
        .join("berth")
        .join("config.toml");
    fs::create_dir_all(&file_path.parent().unwrap()).unwrap();

    TestOutput::new()
        .config_with_path(
            &indoc!(
                r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            "#,
            ),
            &file_path,
        )?
        .args(vec!["--no-tty", "[name]"])?
        .envs(vec![("HOME", tmp_dir.path().to_str().unwrap())])?
        .stderr(format!("Using config file at {:?}\n", file_path))?
        .code(0)?
        .run()?;

    tmp_dir.close().unwrap();
    Ok(())
}

#[test]
fn env_name_with_no_config_in_env() -> Result<()> {
    // Note: TestOutput doesn't inherit envs
    TestOutput::new()
        .config(&indoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            "#,
        ))?
        .args(vec!["--no-tty", "[name]"])?
        .stderr("Could not find config file in $XDG_CONFIG_PATH or $HOME\n")?
        .code(1)?
        .run()
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

#[test]
fn no_tty_prevents_interactive_terminal() -> Result<()> {
    TestHarness::new()
        .config(&indoc!(
            r#"
            image = "alpine:edge"
            init_cmd = "/bin/ash"
            "#,
        ))?
        .args(vec!["--no-tty", "--config-path", "[config_path]", "[name]"])?
        .run(5000)?
        .stdio(&formatdoc!(
            r#"
            Using config file at "[config_path]"
            "#
        ))?
        .success()
}
