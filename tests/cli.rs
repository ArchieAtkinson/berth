use std::{
    fs::{self, File},
    path::PathBuf,
};

use berth::{cli::AppConfig, util::EnvVar};
use tempfile::{NamedTempFile, TempDir};

type EnvVarPairs = Vec<(String, String)>;

fn empty_env_vars() -> EnvVar {
    EnvVar::new(EnvVarPairs::new())
}

#[test]
fn no_commands() {
    let args = vec!["berth"];

    let app_config = AppConfig::new(args, &empty_env_vars());
    assert!(app_config.is_err());

    let err = app_config.err().unwrap();
    assert!(err.contains("the following required arguments were not provided"))
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
    File::create(&file_path).unwrap();

    let fake_env_vars = EnvVar::new(EnvVarPairs::from([(
        "XDG_CONFIG_PATH".to_string(),
        tmp_dir.path().display().to_string(),
    )]));

    let args = vec!["berth", "Name"];

    let app_config = AppConfig::new(args, &fake_env_vars).unwrap();
    assert_eq!(app_config.config_path, file_path);
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
    File::create(&file_path).unwrap();

    let fake_env_vars = EnvVar::new(EnvVarPairs::from([(
        "HOME".to_string(),
        tmp_dir.path().display().to_string(),
    )]));

    let args = vec!["berth", "Name"];

    let app_config = AppConfig::new(args, &fake_env_vars).unwrap();
    assert_eq!(app_config.config_path, file_path);
}

#[test]
fn env_name_with_no_config_in_env() {
    let args = vec!["berth", "Name"];

    let app_config = AppConfig::new(args, &empty_env_vars()).err().unwrap();
    assert_eq!(
        app_config,
        "Could not find config file in $XDG_CONFIG_PATH or $HOME"
    );
}

#[test]
fn vaild_config_file() {
    let config_file = NamedTempFile::new().unwrap();
    let config_file_path = config_file.path().to_str().unwrap();
    let args = vec!["berth", "--config-path", config_file_path, "Name"];

    let app_config = AppConfig::new(args, &empty_env_vars()).unwrap();
    assert_eq!(app_config.env_name, "Name");
    assert_eq!(app_config.config_path.to_str(), Some(config_file_path))
}

#[test]
fn nonexistant_config_file() {
    let not_real_file = PathBuf::from(" ");
    let not_real_file_path = not_real_file.as_path().to_str().unwrap();
    let args = vec!["berth", "--config-path", not_real_file_path, "Name"];

    let app_config = AppConfig::new(args, &empty_env_vars()).err().unwrap();
    let expected_error_text = format!(
        "Could not find config file at provided path: {:?}",
        not_real_file_path
    );
    assert_eq!(app_config, expected_error_text);
}
