use berth::{
    cli::{AppConfig, Commands},
    configuration::{ConfigError, Configuration, Environment},
};
use indoc::{formatdoc, indoc};
use miette::{GraphicalReportHandler, GraphicalTheme, Result};
use pretty_assertions::assert_eq;
use std::{
    fs::{self, File},
    path::PathBuf,
};
use std::{io::Write, path::Path};
use tempfile::{NamedTempFile, TempDir};
use test_utils::TmpEnvVar;

pub mod test_utils;

struct ConfigTest {
    file_path: PathBuf,
    _file: Option<NamedTempFile>,
}

impl ConfigTest {
    pub fn new(config_content: &str) -> Self {
        let config_file = NamedTempFile::new().expect("Failed to create file for config");
        write!(&config_file, "{}", config_content).expect("Failed to write config file");

        ConfigTest {
            file_path: config_file.path().to_path_buf(),
            _file: Some(config_file),
        }
    }

    pub fn from_file(config_path: &Path) -> Self {
        ConfigTest {
            file_path: config_path.to_path_buf(),
            _file: None,
        }
    }

    pub fn get_env(&self, environment: &str) -> Result<Environment> {
        let app_config = AppConfig {
            config_path: self.file_path.clone(),
            command: Commands::Up {
                environment: environment.to_string(),
            },
            cleanup: true,
        };

        Configuration::find_environment_from_configuration(&app_config)
    }

    pub fn file_path(&self) -> &str {
        self.file_path.to_str().unwrap()
    }
}

trait ReportExt {
    fn render(&self) -> String;
}

impl ReportExt for miette::Report {
    fn render(&self) -> String {
        let mut out = String::new();
        GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            .render_report(&mut out, self.as_ref())
            .unwrap();
        out
    }
}

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
    let err = ConfigTest::new(indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
    "#})
    .get_env("Env")
    .unwrap_err()
    .downcast::<ConfigError>()
    .unwrap();

    assert_eq!(
        err,
        ConfigError::RequireDockerfileOrImage("Env".to_string())
    );
}

#[test]
fn both_dockerfile_or_image() {
    let err = ConfigTest::new(indoc! {r#"
        [environment.Env]
        entry_cmd = "hello"
        image = "world"
        dockerfile = "!"
    "#})
    .get_env("Env")
    .unwrap_err()
    .downcast::<ConfigError>()
    .unwrap();
    assert_eq!(err, ConfigError::DockerfileOrImage("Env".to_string()));
}
