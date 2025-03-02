use berth::cli::AppConfig;
use color_eyre::Result;
use indoc::indoc;
use pretty_assertions::assert_eq;
use std::{
    fs::{self},
    path::PathBuf,
};
use tempfile::{NamedTempFile, TempDir};
use test_utils::TestOutput;

pub mod test_utils;

#[test]
fn no_commands() {
    let args = vec!["berth"];

    let app_config = AppConfig::new(&args);
    assert!(app_config.is_err());

    let err = app_config.err().unwrap();
    assert_eq!(
        err.to_string(),
        indoc!(
            r#"
        error: the following required arguments were not provided:
          <ENVIRONMENT>
 
        Usage: berth <ENVIRONMENT>
  
        For more information, try '--help'.
        "#
        )
    )
}

#[test]
fn help() -> Result<()> {
    TestOutput::new()
        .config("")?
        .args(vec!["--help"])?
        .stdout(indoc!(
            r#"
            berth, A CLI to help create development environments without touching repository code

            Usage: berth [OPTIONS] <ENVIRONMENT>

            Arguments:
              <ENVIRONMENT>  The environment to be used

            Options:
                  --config-path <FILE>  Path to config file
                  --cleanup             Deletes container on exit
                  --build               Build/rebuild the environment instead of starting it
                  --view                View environment definition after it has been parsed by berth
              -h, --help                Print help
  
            "#
        ))?
        .code(0)?
        .run()
}

#[test]
fn env_name_with_config_in_xdg_config_path() -> Result<()> {
    let tmp_dir = TempDir::new().unwrap();
    let file_path = tmp_dir
        .path()
        .join(".config")
        .join("berth")
        .join("config.toml");
    fs::create_dir_all(file_path.parent().unwrap()).unwrap();

    TestOutput::new()
        .config_with_path(
            indoc!(
                r#"
            image = "alpine:edge"
            entry_cmd = "true"
            "#,
            ),
            &file_path,
        )?
        .args(vec!["[name]"])?
        .envs(vec![("XDG_CONFIG_HOME", tmp_dir.path().to_str().unwrap())])?
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
    fs::create_dir_all(file_path.parent().unwrap()).unwrap();

    TestOutput::new()
        .config_with_path(
            indoc!(
                r#"
            image = "alpine:edge"
            entry_cmd = "/bin/ash"
            "#,
            ),
            &file_path,
        )?
        .args(vec!["[name]"])?
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
        .config(indoc!(
            r#"
            image = "alpine:edge"
            entry_cmd = "/bin/ash"
            "#,
        ))?
        .args(vec!["[name]"])?
        .stderr(indoc!(
            r#"
            Error:   × Could not find config file in $XDG_CONFIG_HOME or $HOME
            
            "#
        ))?
        .code(1)?
        .run()
}

#[test]
fn valid_config_file() {
    let config_file = NamedTempFile::new().unwrap();
    let config_file_path = config_file.path().to_str().unwrap();
    let args = vec!["berth", "--config-path", config_file_path, "Name"];

    let app_config = AppConfig::new(args).unwrap();
    assert_eq!(app_config.environment, "Name".to_string());
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
    let args = vec!["berth", "--bad-command", "Env"];

    let app_config = AppConfig::new(args).err().unwrap();
    assert_eq!(
        app_config.to_string(),
        indoc!(
            r#"
        error: unexpected argument '--bad-command' found
        
          tip: to pass '--bad-command' as a value, use '-- --bad-command'

        Usage: berth [OPTIONS] <ENVIRONMENT>

        For more information, try '--help'.
        "#
        )
    );
}

#[test]
fn no_two_actions_allowed() {
    let args = vec!["berth", "--build", "--view", "Env"];

    let app_config = AppConfig::new(args).err().unwrap();
    assert_eq!(
        app_config.to_string(),
        indoc!(
            r#"
        error: the argument '--build' cannot be used with '--view'

        Usage: berth --build <ENVIRONMENT>

        For more information, try '--help'.
        "#
        )
    );
}
