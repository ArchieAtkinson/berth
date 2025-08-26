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
fn simple_preset() {
    let config = ConfigTest::new(
        r#"
        [preset.Preset]
        image = "image"
        entry_cmd = "init"
        entry_options = ["entry_options"]
        exec_options = ["exec_options"]
        create_options = ["create_options"]

        [environment.Env]
        presets = ["Preset"]
    "#,
    );

    let env1 = config.get_env("Env").unwrap();

    assert_eq!(env1.image, "image");
    assert_eq!(env1.entry_cmd, "init");
    assert_eq!(env1.entry_options, vec!["entry_options"]);
    assert_eq!(env1.exec_options, vec!["exec_options"]);
    assert_eq!(env1.create_options, vec!["create_options"]);
}

#[test]
fn multiple_preset() {
    let config = ConfigTest::new(
        r#"
        [preset.Preset1]
        image = "image1"
        entry_options = ["entry_options1"]
        exec_options = ["exec_options1"]
        create_options = ["create_options1"]

        [preset.Preset2]
        entry_cmd = "init2"
        entry_options = ["entry_options2"]
        exec_options = ["exec_options2"]
        create_options = ["create_options2"]

        [environment.Env]
        presets = ["Preset1", "Preset2"]
    "#,
    );

    let env = config.get_env("Env").unwrap();

    assert_eq!(env.image, "image1");
    assert_eq!(env.entry_cmd, "init2");
    assert_eq!(env.entry_options, vec!["entry_options1", "entry_options2"]);
    assert_eq!(env.exec_options, vec!["exec_options1", "exec_options2"]);
    assert_eq!(
        env.create_options,
        vec!["create_options1", "create_options2"]
    );
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
fn view_parsed_config() {
    let config = ConfigTest::new(
        r#"
        [preset.Preset1]
        image = "image1"
        entry_options = ["entry_options1"]
        exec_options = ["exec_options1"]
        create_options = ["create_options1"]

        [preset.Preset2]
        entry_cmd = "init2"
        entry_options = ["entry_options2"]
        exec_options = ["exec_options2"]
        create_options = ["create_options2"]

        [environment.Env]
        presets = ["Preset1", "Preset2"]
    "#,
    );

    let env_view = config.get_env("Env").unwrap().view().unwrap();

    assert_eq!(
        env_view,
        indoc!(
            r#"
        [environment.Env]
        image = "image1"
        entry_cmd = "init2"
        entry_options = ["entry_options1", "entry_options2"]
        exec_options = ["exec_options1", "exec_options2"]
        create_options = ["create_options1", "create_options2"]
        "#
        )
    );
}

#[test]
fn test_intermediate_view_with_env_vars() {
    let dockerfile = NamedTempFile::new().expect("Failed to create temporary dockerfile");
    let dockerfile_path = dockerfile.path().to_str().unwrap();

    let entry_option = TmpEnvVar::new("/test/path");
    let create_option = TmpEnvVar::new("/custom/docker");

    let config = ConfigTest::new(&formatdoc!(
        r#"
        [environment.EnvExpansion]
        dockerfile = "{}"
        entry_cmd = "bash"
        entry_options = ["-v ${{{}}}:/data"]
        create_options = ["-v ${{{}}}:/mount"]
        "#,
        dockerfile_path,
        entry_option.name(),
        create_option.name()
    ));

    let env = config.get_env("EnvExpansion").unwrap();
    let view_output = env.view().unwrap();

    let expected = formatdoc!(
        r#"
        [environment.EnvExpansion]
        dockerfile = "{}"
        entry_cmd = "bash"
        entry_options = ["-v {}:/data"]
        create_options = ["-v {}:/mount"]
        "#,
        dockerfile_path,
        entry_option.value(),
        create_option.value()
    );

    assert_eq!(view_output, expected);
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
          configuration::environment::validation
 
            × Malformed Environment
             ╭─[{}:1:1]
           1 │ ╭─▶ [environment.Env]
           2 │ ├─▶ image = "1"
             · ╰──── An environment requires a 'entry_cmd' field
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

#[test]
fn build_context_and_no_dockerfile() {
    let config = ConfigTest::new(indoc! {r#"
        [environment.Env]
        image = "foo"
        entry_cmd = "hello"
        build_context = "world"
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
              2 │ │   image = "foo"
              3 │ │   entry_cmd = "hello"
              4 │ ├─▶ build_context = "world"
                · ╰──── 'build_context' can only be used with a 'dockerfile'
                ╰────
            "#,
            config.file_path()
        )
    );
}

#[test]
fn preset_not_found() {
    let config = ConfigTest::new(indoc! {r#"
        [preset.preset]
        entry_options = ["a"]
        
        [environment.Env]
        entry_cmd = "hello"
        image = "world"
        presets = ["preset", "different_preset"]
    "#});
    let err = config.get_env("Env").unwrap_err().render();
    assert_eq!(
        err,
        formatdoc!(
            r#"
             configuration::preset::unknown
 
               × Unknown Preset
                ╭─[{}:7:22]
              6 │ image = "world"
              7 │ presets = ["preset", "different_preset"]
                ·                      ─────────┬────────
                ·                               ╰── Failed to find provided preset
                ╰────
            "#,
            config.file_path()
        )
    );
}

#[test]
fn multiple_unique_fields_from_presets() {
    let config = ConfigTest::new(
        r#"
        [preset.Preset1]
        image = "image1"

        [preset.Preset2]
        image = "image2"

        [environment.Env]
        image = "image"
        entry_cmd = "init"
        presets = ["Preset1", "Preset2"]
    "#,
    );

    let err = config.get_env("Env").unwrap_err().render();
    assert_eq!(
        err,
        formatdoc!(
            r#"
             configuration::preset::duplication
 
               × Duplicate Fields From Presets
                 ╭─[{}:3:9]
               2 │         [preset.Preset1]
               3 │         image = "image1"
                 ·         ────────┬───────
                 ·                 ╰── instance 2
               4 │ 
               5 │         [preset.Preset2]
               6 │         image = "image2"
                 ·         ────────┬───────
                 ·                 ╰── instance 3
               7 │ 
               8 │         [environment.Env]
               9 │         image = "image"
                 ·         ───────┬───────
                 ·                ╰── instance 1
              10 │         entry_cmd = "init"
              11 │         presets = ["Preset1", "Preset2"]
                 ·                   ───────────┬──────────
                 ·                              ╰── Preset(s) causing duplicate 'image' field
              12 │     
                 ╰────
            "#,
            config.file_path()
        )
    );
}
