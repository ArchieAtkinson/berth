use std::{
    collections::HashMap,
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
};

use envmnt::{ExpandOptions, ExpansionType};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::cli::{AppConfig, Commands};

#[derive(Debug, Error, PartialEq)]
pub enum ConfigError {
    #[error("{message}")]
    TomlParse { message: String },

    #[error("Environment '{environment}' has specified dockerfile or image")]
    DockerfileOrImage { environment: String },

    #[error("Environment '{environment}' has not specified a dockerfile or image")]
    RequireDockerfileOrImage { environment: String },

    #[error("Can't find dockerfile at: {path}")]
    BadDockerfilePath { path: String },

    #[error("Proved environment, '{name}' is not in loaded config")]
    ProvidedEnvNameNotInConfig { name: String },

    #[error("Couldn't read provided dockerfile, '{path}', for hashing")]
    FailedToInteractWithDockerfile { path: String },
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
    pub fn find_environment_from_configuration(
        file: &str,
        app: &AppConfig,
    ) -> Result<Environment, ConfigError> {
        match toml::from_str::<TomlConfiguration>(file) {
            Ok(mut config) => {
                Self::check_toml_environments_are_valid(&config.environments)?;
                Ok(Self::create_environment(&mut config.environments, app)?)
            }
            Err(e) => Err(ConfigError::TomlParse {
                message: e.to_string(),
            }),
        }
    }

    fn check_toml_environments_are_valid(envs: &TomlEnvs) -> Result<(), ConfigError> {
        for (name, env) in envs {
            match (env.provided_image.is_empty(), env.dockerfile.is_empty()) {
                (true, true) => {
                    return Err(ConfigError::RequireDockerfileOrImage {
                        environment: name.clone(),
                    })
                }
                (false, false) => {
                    return Err(ConfigError::DockerfileOrImage {
                        environment: name.clone(),
                    })
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn create_environment(
        config: &mut TomlEnvs,
        app: &AppConfig,
    ) -> Result<Environment, ConfigError> {
        let name = match app.command.clone() {
            Commands::Up { environment: e } => e,
            Commands::Build { environment: e } => e,
        };

        let mut env = config
            .remove(&name)
            .ok_or(ConfigError::ProvidedEnvNameNotInConfig {
                name: name.to_string(),
            })?;

        Self::expand_environment_variables(&mut env);

        let (image, dockerfile) = if env.provided_image.is_empty() {
            let dockerfile_path = Self::parse_dockerfile(&env.dockerfile, &app.config_path)?;
            let image_name = Self::generate_image_name(&name, &dockerfile_path)?;
            (image_name, Some(dockerfile_path))
        } else {
            (env.provided_image, None)
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

    fn expand_environment_variables(env: &mut TomlEnvironment) {
        Self::expand_env_vars(&mut env.entry_options);
        Self::expand_env_vars(&mut env.exec_options);
        Self::expand_env_vars(&mut env.create_options);
    }

    fn parse_dockerfile(dockerfile: &str, config_path: &Path) -> Result<PathBuf, ConfigError> {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        let dockerfile = envmnt::expand(dockerfile, Some(options));

        let path = Path::new(&dockerfile);

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let config_dir =
                config_path
                    .parent()
                    .ok_or(ConfigError::FailedToInteractWithDockerfile {
                        path: path.display().to_string(),
                    })?;
            config_dir.join(path)
        };

        if !resolved.exists() || !resolved.is_file() {
            return Err(ConfigError::BadDockerfilePath {
                path: resolved.display().to_string(),
            });
        }

        Ok(resolved)
    }

    fn generate_image_name(name: &str, path: &Path) -> Result<String, ConfigError> {
        let create_error = |path: &Path| -> ConfigError {
            ConfigError::FailedToInteractWithDockerfile {
                path: path.display().to_string(),
            }
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

    fn expand_env_vars(vec: &mut [String]) {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        vec.iter_mut()
            .for_each(|s| *s = envmnt::expand(s, Some(options)));
    }
}
