use berth::configuration::{Configuration, ConfigurationError};
use indoc::{formatdoc, indoc};
use pretty_assertions::assert_eq;
use test_utils::TmpEnvVar;

pub mod test_utils;

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
    let envs = Configuration::new(&content).unwrap().environments;

    assert!(envs.contains_key("Env1"));
    assert!(envs.contains_key("Env2"));

    assert_eq!(envs.get("Env1").unwrap().image, "image1");
    assert_eq!(envs.get("Env2").unwrap().image, "image2");

    assert_eq!(envs.get("Env1").unwrap().entry_cmd, "init1");
    assert_eq!(envs.get("Env2").unwrap().entry_cmd, "init2");
}

#[test]
fn unknown_field() {
    let content = indoc! {r#"
        [environment.Env]
        unknown = "Should Fail"
    "#};
    let configuration = Configuration::new(&content);
    let err_str = configuration.unwrap_err().to_string();
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
fn dockerfile() {
    let content = indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        dockerfile = "world"
    "#};
    let binding = Configuration::new(&content).unwrap();
    let env = binding.environments.get("Env").unwrap();
    assert_eq!(env.dockerfile, "world");
}

#[test]
fn image() {
    let content = indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        image = "world"
    "#};
    let binding = Configuration::new(&content).unwrap();
    let env = binding.environments.get("Env").unwrap();
    assert_eq!(env.image, "world");
}

#[test]
fn no_dockerfile_or_image() {
    let content = indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
    "#};
    let err = Configuration::new(&content).unwrap_err();
    assert_eq!(
        err,
        ConfigurationError::RequireDockerfileOrImage {
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
    let err = Configuration::new(&content).unwrap_err();
    assert_eq!(
        err,
        ConfigurationError::DockerfileOrImage {
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

    let mut configuration = Configuration::new(&content).unwrap();
    let env = configuration.environments.remove("Env").unwrap();

    assert_eq!(&env.create_options[0], &var.value());
    assert_eq!(&env.exec_options[0], &var.value());
    assert_eq!(&env.entry_options[0], &var.value());
}
