use indoc::{formatdoc, indoc};
use pretty_assertions::assert_eq;
use std::fs::{self, File};
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};
use test_utils::{ConfigTest, ReportExt, TmpEnvVar};
pub mod test_utils;

#[test]
fn multiple_envs_in_config() {
    let config = ConfigTest::new(
        r#"
        [environment.Env1]
        image = "image1"
        entry_cmd = "init1"

        [environment.Env2]
        image = "image2"
        entry_cmd = "init2"
    "#,
    );

    let env1 = config.get_env("Env1").unwrap();
    let env2 = config.get_env("Env2").unwrap();

    assert_eq!(env1.image, "image1");
    assert_eq!(env2.image, "image2");

    assert_eq!(env1.entry_cmd, "init1");
    assert_eq!(env2.entry_cmd, "init2");
}

#[test]
fn dockerfile_absolute_path() {
    let dockerfile = NamedTempFile::new().expect("Failed to create temporary file for config");
    let dockerfile_path = dockerfile.path().to_str().unwrap();

    let env = ConfigTest::new(&formatdoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        dockerfile = "{}"
        "#,
        dockerfile_path
    })
    .get_env("Env")
    .unwrap();
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

    let config_file = File::create(&config_path).unwrap();
    File::create(&docker_path).unwrap();

    let content = indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        dockerfile = "dockerfile"
        "#};

    write!(&config_file, "{}", content).unwrap();

    let env = ConfigTest::from_file(&config_path).get_env("Env").unwrap();
    assert_eq!(env.dockerfile.unwrap(), docker_path);

    tmp_dir.close().unwrap();
}

#[test]
fn env_vars_in_options() {
    let var = TmpEnvVar::new("/dir");
    let env = ConfigTest::new(&formatdoc!(
        r#"
        [environment.Env]
        image = "image"
        entry_cmd = "cmd"
        create_options = ["${0}"]
        exec_options = ["${0}"]
        entry_options = ["${0}"]
    "#,
        var.name()
    ))
    .get_env("Env")
    .unwrap();

    assert_eq!(&env.create_options[0], &var.value());
    assert_eq!(&env.exec_options[0], &var.value());
    assert_eq!(&env.entry_options[0], &var.value());
}

#[test]
fn environment_not_in_config() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        image = "image"
        entry_cmd = "cmd"
        create_options = ["create options"]
        exec_options = ["exec option"]
        entry_options = ["entry option"]
    "#});

    let err = config.get_env("NotEnv").unwrap_err();
    assert_eq!(
        err.render(),
        formatdoc! {
        r#"
         configuration::environment::search
 
           × Environment Not Present
            ╭─[{}:1:1]
          1 │ ╭─▶ [environment.Env]
          2 │ │   image = "image"
          3 │ │   entry_cmd = "cmd"
          4 │ │   create_options = ["create options"]
          5 │ │   exec_options = ["exec option"]
          6 │ ├─▶ entry_options = ["entry option"]
            · ╰──── Failed to find provided environment 'NotEnv' in config
            ╰────
        "#, config.file_path()
        }
    );
}

#[test]
fn non_existent_dockerfile() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        dockerfile = "/tmp/file_that_is_not_real"
        entry_cmd = "cmd"
    "#});

    let err = config.get_env("Env").unwrap_err();

    assert_eq!(
        err.render(),
        formatdoc!(
            r#"
             configuration::environment::dockerfile

               × Nonexistent Dockerfile
                ╭─[{}:2:14]
              1 │ [environment.Env]
              2 │ dockerfile = "/tmp/file_that_is_not_real"
                ·              ──────────────┬─────────────
                ·                            ╰── Could not find dockerfile
              3 │ entry_cmd = "cmd"
                ╰────
            "#,
            config.file_path()
        )
    );
}

#[test]
fn invalid_field_type_in_config() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        image = 5
    "#});

    let err = config.get_env("").unwrap_err();

    assert_eq!(
        err.render(),
        formatdoc!(
            r#"
            configuration::parsing

              × Malformed TOML
               ╭─[{}:2:9]
             1 │ [environment.Env]
             2 │ image = 5
               ·         ┬
               ·         ╰── invalid type: integer `5`, expected a string
               ╰────
        "#,
            config.file_path()
        )
    );
}

#[test]
fn unknown_field_in_config() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        unknown = "Should Fail"
    "#});

    let err = config.get_env("").unwrap_err();
    assert_eq!(
        err.render(),
        formatdoc! {
        r#"
        configuration::parsing

          × Malformed TOML
           ╭─[{}:2:1]
         1 │ [environment.Env]
         2 │ unknown = "Should Fail"
           · ───┬───
           ·    ╰── Unknown field
           ╰────
        "#, config.file_path()
        }
    );
}

#[test]
fn duplicate_field_in_config() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        image = "1"
        image = "2"
    "#});

    let err = config.get_env("").unwrap_err();
    assert_eq!(
        err.render(),
        formatdoc! {
        r#"
          configuration::parsing
 
            × Malformed TOML
             ╭─[{}:3:1]
           2 │ image = "1"
           3 │ image = "2"
             · ┬
             · ╰── duplicate key `image` in table `environment.Env`
             ╰────
        "#, config.file_path()
        }
    );
}

#[test]
fn missing_field_in_config() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        image = "1"
    "#});

    let err = config.get_env("").unwrap_err();
    assert_eq!(
        err.render(),
        formatdoc! {
        r#"
          configuration::parsing
 
            × Malformed TOML
             ╭─[{}:1:1]
           1 │ ╭─▶ [environment.Env]
           2 │ ├─▶ image = "1"
             · ╰──── missing field `entry_cmd`
             ╰────
        "#, config.file_path()
        }
    );
}

#[test]
fn no_dockerfile_or_image_in_config() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
    "#});
    let err = config.get_env("Env").unwrap_err().render();
    assert_eq!(
        err,
        formatdoc!(
            r#"
             configuration::environment::validation

               × Malformed Environment
                ╭─[{}:1:1]
              1 │ ╭─▶ [environment.Env]
              2 │ ├─▶ entry_cmd = "hello"
                · ╰──── An environment requires an 'image' or 'dockerfile' field
                ╰────
            "#,
            config.file_path()
        )
    );
}

#[test]
fn both_dockerfile_or_image() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        image = "world"
        dockerfile = "!"
    "#});
    let err = config.get_env("Env").unwrap_err().render();
    assert_eq!(
        err,
        formatdoc!(
            r#"
             configuration::environment::validation

               × Malformed Environment
                ╭─[{}:1:1]
              1 │ ╭─▶ [environment.Env]
              2 │ │   entry_cmd = "hello"
              3 │ │   image = "world"
              4 │ ├─▶ dockerfile = "!"
                · ╰──── An environment can only have an 'image' or 'dockerfile' field
                ╰────
            "#,
            config.file_path()
        )
    );
}
