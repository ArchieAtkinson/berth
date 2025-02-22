use envmnt::{ExpandOptions, ExpansionType};
use miette::{miette, Diagnostic, NamedSource, Result, SourceSpan};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::cli::{AppConfig, Commands};

#[derive(Debug, Error, PartialEq, Diagnostic)]
pub enum ConfigError {
    #[error("Malformed TOML")]
    #[diagnostic(code(configuration::parsing))]
    TomlParse {
        msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{msg}")]
        span: SourceSpan,
    },

    #[error("Malformed Environment")]
    #[diagnostic(code(configuration::environment::validation))]
    EnvironmentValidation {
        msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{msg}")]
        span: SourceSpan,
    },

    #[error("Environment Not Present")]
    #[diagnostic(code(configuration::environment::search))]
    EnvironmentSearch {
        msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{msg}")]
        span: SourceSpan,
    },

    #[error("Nonexistent Dockerfile")]
    #[diagnostic(code(configuration::environment::dockerfile))]
    MissingDockerfile {
        #[source_code]
        input: NamedSource<String>,
        #[label("Could not find dockerfile")]
        span: SourceSpan,
    },

    #[error("Environment '{0}' has specified dockerfile or image")]
    DockerfileOrImage(String),

    #[error("Environment '{0}' has not specified a dockerfile or image")]
    RequireDockerfileOrImage(String),

    #[error("Can't find dockerfile at: {0}")]
    BadDockerfilePath(String),

    #[error("Proved environment, '{0}' is not in loaded config")]
    ProvidedEnvNameNotInConfig(String),

    #[error("Couldn't read provided dockerfile, '{0}', for hashing")]
    FailedToInteractWithDockerfile(String),
}

#[derive(Debug, Deserialize, Hash)]
#[serde(deny_unknown_fields)]
pub struct TomlEnvironment {
    entry_cmd: String,

    #[serde(default)]
    #[serde(rename = "image")]
    provided_image: String,

    #[serde(default)]
    dockerfile: String,

    #[serde(default)]
    entry_options: Vec<String>,

    #[serde(default)]
    exec_cmds: Vec<String>,

    #[serde(default)]
    exec_options: Vec<String>,

    #[serde(default)]
    create_options: Vec<String>,
}

type TomlEnvs = HashMap<String, TomlEnvironment>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomlConfiguration {
    #[serde(rename = "environment")]
    pub environments: TomlEnvs,
}

pub struct Configuration {}

#[derive(Hash, Debug)]
pub struct Environment {
    pub name: String,
    pub image: String,
    pub dockerfile: Option<PathBuf>,
    pub entry_cmd: String,
    pub entry_options: Vec<String>,
    pub exec_cmds: Vec<String>,
    pub exec_options: Vec<String>,
    pub create_options: Vec<String>,
}

impl Configuration {
    pub fn find_environment_from_configuration(app: &AppConfig) -> Result<Environment> {
        let content = fs::read_to_string(&app.config_path).expect("Failed to read config file");

        Self::parse_toml(&content, &app.config_path)
            .and_then(|envs| Self::validate_environments(envs, &content, &app.config_path))
            .and_then(|envs| Self::create_environment(envs, &content, &app))
    }

    fn parse_toml(content: &str, path: &Path) -> Result<TomlEnvs> {
        return match toml_edit::de::from_str::<TomlConfiguration>(&content) {
            Ok(config) => Ok(config.environments),
            Err(error) => {
                let span = error.span().unwrap();

                let label_message = match error.message() {
                    s if s.contains("missing field") => error.message(),
                    s if s.contains("unknown field") => "Unknown field",
                    s if s.contains("invalid type") => error.message(),
                    s if s.contains("duplicate key") => error.message(),
                    _ => &format!("Unexpected TOML Error {:?}", error.message()),
                };

                Err(ConfigError::TomlParse {
                    input: NamedSource::new(path.to_str().unwrap(), content.to_string()),
                    span: span.into(),
                    msg: label_message.to_string(),
                }
                .into())
            }
        };
    }

    fn validate_environments(envs: TomlEnvs, content: &str, path: &Path) -> Result<TomlEnvs> {
        let create_error = move |env_name: &str, message: &str| -> Result<miette::Report> {
            let doc: toml_edit::ImDocument<String> = content.parse().unwrap();

            let span = doc
                .get("environment")
                .and_then(|env| env.as_table())
                .and_then(|table| table.get(&env_name))
                .and_then(|item| item.span())
                .ok_or_else(|| miette!("Unknown Parse Error"))?;

            Ok(ConfigError::EnvironmentValidation {
                input: NamedSource::new(path.to_str().unwrap(), content.to_string()),
                span: span.into(),
                msg: message.to_string(),
            }
            .into())
        };

        for (name, env) in &envs {
            match (env.provided_image.is_empty(), env.dockerfile.is_empty()) {
                (true, true) => {
                    return Err(create_error(
                        &name,
                        "An environment requires an 'image' or 'dockerfile' field",
                    )?)
                }
                (false, false) => {
                    return Err(create_error(
                        &name,
                        "An environment can only have an 'image' or 'dockerfile' field",
                    )?)
                }
                _ => (),
            }
        }

        Ok(envs)
    }

    fn create_environment(
        mut envs: TomlEnvs,
        content: &str,
        app: &AppConfig,
    ) -> Result<Environment> {
        let name = match app.command.clone() {
            Commands::Up { environment: e } => e,
            Commands::Build { environment: e } => e,
        };

        let mut env = match envs.remove(&name) {
            Some(env) => env,
            None => {
                return Err(ConfigError::EnvironmentSearch {
                    input: NamedSource::new(app.config_path.to_str().unwrap(), content.to_string()),
                    span: (0, content.len()).into(),
                    msg: format!("Failed to find provided environment '{}' in config", &name),
                }
                .into())
            }
        };

        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        [
            &mut env.entry_options,
            &mut env.exec_options,
            &mut env.create_options,
        ]
        .iter_mut()
        .for_each(|vec| {
            vec.iter_mut()
                .for_each(|s| *s = envmnt::expand(s, Some(options)))
        });

        let (image, dockerfile) = match env.provided_image.as_str() {
            "" => {
                let dockerfile_path =
                    Self::validate_dockerfile(&env.dockerfile, content, &name, &app.config_path)?;
                let image_name = Self::generate_image_name(&name, &dockerfile_path)?;
                (image_name, Some(dockerfile_path))
            }
            _ => (env.provided_image, None),
        };

        let mut env = Environment {
            name: name.to_string(),
            image,
            dockerfile,
            entry_cmd: env.entry_cmd,
            entry_options: env.entry_options,
            exec_cmds: env.exec_cmds,
            exec_options: env.exec_options,
            create_options: env.create_options,
        };

        let mut hasher = DefaultHasher::new();
        env.hash(&mut hasher);
        env.name = format!("{}-{}-{:016x}", "berth", &name, hasher.finish());

        Ok(env)
    }

    fn validate_dockerfile(
        dockerfile: &str,
        content: &str,
        env_name: &str,
        config_path: &Path,
    ) -> Result<PathBuf> {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        let dockerfile = envmnt::expand(dockerfile, Some(options));

        let path = Path::new(&dockerfile);

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            config_path
                .parent()
                .ok_or_else(|| {
                    ConfigError::FailedToInteractWithDockerfile(path.display().to_string())
                })?
                .join(path)
        };

        if !resolved.exists() || !resolved.is_file() {
            let doc: toml_edit::ImDocument<String> = content
                .parse()
                .map_err(|_| miette!("Unknown Parse Error"))?;

            let span = doc
                .get("environment")
                .and_then(|env| env.as_table())
                .and_then(|envs| envs.get(&env_name))
                .and_then(|env| env.get("dockerfile"))
                .and_then(|item| item.span())
                .ok_or_else(|| miette!("Unknown Parse Error"))?;

            return Err(ConfigError::MissingDockerfile {
                input: NamedSource::new(config_path.to_str().unwrap(), content.to_string()),
                span: span.into(),
            }
            .into());
        }

        Ok(resolved)
    }

    fn generate_image_name(name: &str, path: &Path) -> Result<String> {
        let create_error = |path: &Path| -> miette::Report {
            ConfigError::FailedToInteractWithDockerfile(path.display().to_string()).into()
        };

        let path = fs::canonicalize(path).map_err(|_| create_error(path))?;
        let mut file = File::open(&path).map_err(|_| create_error(&path))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 1024];

        loop {
            let bytes_read = file.read(&mut buffer).map_err(|_| create_error(&path))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!(
            "{}-{}-{:016x}",
            "berth",
            name.to_lowercase(),
            hasher.finalize()
        ))
    }
}
