use std::{collections::HashMap, path::Path};

use envmnt::{ExpandOptions, ExpansionType};
use serde::Deserialize;
use thiserror::Error;

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
}

#[derive(Debug, Deserialize, Hash)]
#[serde(deny_unknown_fields)]
pub struct TomlEnvironment {
    pub entry_cmd: String,

    #[serde(default)]
    pub image: String,

    #[serde(default)]
    pub dockerfile: String,

    #[serde(default)]
    pub entry_options: Vec<String>,

    #[serde(default)]
    pub exec_cmds: Vec<String>,

    #[serde(default)]
    pub exec_options: Vec<String>,

    #[serde(default)]
    pub create_options: Vec<String>,
}

type TomlEnvs = HashMap<String, TomlEnvironment>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    #[serde(rename = "environment")]
    pub environments: TomlEnvs,
}

impl Configuration {
    pub fn new(file: &str, config_path: &Path) -> Result<Configuration, ConfigError> {
        match toml::from_str::<Configuration>(file) {
            Ok(v) => Ok(Configuration {
                environments: Self::parse_envs(v.environments, config_path)?,
            }),
            Err(e) => Err(ConfigError::TomlParse {
                message: e.to_string(),
            }),
        }
    }

    fn parse_envs(mut environments: TomlEnvs, config_path: &Path) -> Result<TomlEnvs, ConfigError> {
        for (name, env) in environments.iter_mut() {
            Self::expand_env_vars(&mut env.entry_options);
            Self::expand_env_vars(&mut env.exec_options);
            Self::expand_env_vars(&mut env.create_options);

            match (env.image.is_empty(), env.dockerfile.is_empty()) {
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
                (true, false) => Self::parse_dockerfile(&mut env.dockerfile, &config_path)?,
                _ => (),
            }
        }
        Ok(environments)
    }

    fn parse_dockerfile(dockerfile: &mut String, config_path: &Path) -> Result<(), ConfigError> {
        let path = Path::new(&dockerfile);

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let config_dir = config_path.parent().unwrap();
            config_dir.join(path)
        };

        if !resolved.exists() || !resolved.is_file() {
            return Err(ConfigError::BadDockerfilePath {
                path: resolved.display().to_string(),
            });
        }

        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);
        *dockerfile = envmnt::expand(resolved.to_str().unwrap(), Some(options));

        Ok(())
    }

    fn expand_env_vars(vec: &mut Vec<String>) {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        vec.iter_mut()
            .for_each(|s| *s = envmnt::expand(&s, Some(options)));
    }
}
