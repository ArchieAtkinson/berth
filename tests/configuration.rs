use std::{
    fs::{self, File},
    path::PathBuf,
};

use berth::{
    cli::{AppConfig, Commands},
    configuration::{ConfigError, Configuration, Environment},
};
use indoc::{formatdoc, indoc};
use pretty_assertions::assert_eq;
use tempfile::{NamedTempFile, TempDir};
use test_utils::TmpEnvVar;

pub mod test_utils;

fn get_environment(
    config: &str,
    env: &str,
    config_path: Option<PathBuf>,
) -> Result<Environment, ConfigError> {
    let app_config = AppConfig {
        config_path: config_path.unwrap_or_default(),
        command: Commands::Up {
            environment: env.to_string(),
        },
        cleanup: true,
    };
    Configuration::find_environment_from_configuration(config, &app_config)
}

#[test]
fn basic_configuration_file() {
    let content = r#"
        [environment.Env1]
        image = "image1"
        entry_cmd = "init1"

        [environment.Env2]
        image = "image2"
        entry_cmd = "init2"
    "#;
    let env1 = get_environment(content, "Env1", None).unwrap();
    let env2 = get_environment(content, "Env2", None).unwrap();

    assert_eq!(env1.image, "image1");
    assert_eq!(env2.image, "image2");

    assert_eq!(env1.entry_cmd, "init1");
    assert_eq!(env2.entry_cmd, "init2");
}

#[test]
fn unknown_field() {
    let content = indoc! {r#"
        [environment.Env]
        unknown = "Should Fail"
    "#};

    let env = get_environment(content, "Env2", None);
    let err_str = env.unwrap_err().to_string();
    assert_eq!(
        err_str,
        indoc! {
        r#"
        TOML parse error at line 2, column 1
          |
        2 | unknown = "Should Fail"
          | ^^^^^^^
        unknown field `unknown`, expected one of `entry_cmd`, `image`, `dockerfile`, `entry_options`, `exec_cmds`, `exec_options`, `create_options`
        "#
        }
    );
}

#[test]
fn dockerfile_absolute_path() {
    let dockerfile = NamedTempFile::new().expect("Failed to create temporary file for config");
    let dockerfile_path = dockerfile.path().to_str().unwrap();

    let content = formatdoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        dockerfile = "{}"
        "#,
        dockerfile_path
    };

    let env = get_environment(&content, "Env", None).unwrap();
    assert_eq!(env.dockerfile.unwrap(), dockerfile.path());

    dockerfile.close().unwrap();
}

#[test]
fn dockerfile_relative_to_config_file() {
    let tmp_dir = TempDir::new().unwrap();
    let config_dir = tmp_dir.path().join("configdir");
    fs::create_dir_all(&config_dir).unwrap();

    let config_path = config_dir.as_path().join("config.toml");
    let docker_path = config_dir.as_path().join("dockerfile");

    File::create(&config_path).unwrap();
    File::create(&docker_path).unwrap();

    let content = indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        dockerfile = "dockerfile"
        "#};

    let env = get_environment(content, "Env", Some(config_path.to_path_buf())).unwrap();
    assert_eq!(env.dockerfile.unwrap(), docker_path);

    tmp_dir.close().unwrap();
}

#[test]
fn image() {
    let content = indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        image = "world"
    "#};
    let env = get_environment(content, "Env", None).unwrap();
    assert_eq!(env.image, "world");
}

#[test]
fn no_dockerfile_or_image() {
    let content = indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
    "#};
    let err = get_environment(content, "Env", None).unwrap_err();
    assert_eq!(
        err,
        ConfigError::RequireDockerfileOrImage {
            environment: "Env".to_string()
        }
    );
}

#[test]
fn both_dockerfile_or_image() {
    let content = indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        image = "world"
        dockerfile = "!"
    "#};
    let err = get_environment(content, "Env", None).unwrap_err();
    assert_eq!(
        err,
        ConfigError::DockerfileOrImage {
            environment: "Env".to_string()
        }
    );
}

#[test]
fn env_vars_in_options() {
    let var = TmpEnvVar::new("/dir");
    let content = formatdoc!(
        r#"
        [environment.Env]
        image = "image"
        entry_cmd = "cmd"
        create_options = ["${0}"]
        exec_options = ["${0}"]
        entry_options = ["${0}"]
    "#,
        var.name()
    );

    let env = get_environment(&content, "Env", None).unwrap();
    assert_eq!(&env.create_options[0], &var.value());
    assert_eq!(&env.exec_options[0], &var.value());
    assert_eq!(&env.entry_options[0], &var.value());
}
