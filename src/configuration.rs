use std::collections::HashMap;

use envmnt::{ExpandOptions, ExpansionType};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ConfigurationError {
    #[error("{message}")]
    TomlParse { message: String },

    #[error("Environment '{environment}' has specified dockerfile or image")]
    DockerfileOrImage { environment: String },

    #[error("Environment '{environment}' has not specified a dockerfile or image")]
    RequireDockerfileOrImage { environment: String },
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
    pub fn new(file: &str) -> Result<Configuration, ConfigurationError> {
        match toml::from_str::<Configuration>(file) {
            Ok(v) => Ok(Configuration {
                environments: Self::parse_envs(v.environments)?,
            }),
            Err(e) => Err(ConfigurationError::TomlParse {
                message: e.to_string(),
            }),
        }
    }

    fn parse_envs(environments: TomlEnvs) -> Result<TomlEnvs, ConfigurationError> {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        let envs: TomlEnvs = environments
            .into_iter()
            .map(|(name, mut env)| {
                env.dockerfile = envmnt::expand(&env.dockerfile, Some(options));
                Self::expand_env_vars(&mut env.entry_options);
                Self::expand_env_vars(&mut env.exec_options);
                Self::expand_env_vars(&mut env.create_options);
                (name, env)
            })
            .collect();

        for env in &envs {
            match (env.1.image.is_empty(), env.1.dockerfile.is_empty()) {
                (true, true) => {
                    return Err(ConfigurationError::RequireDockerfileOrImage {
                        environment: env.0.clone(),
                    })
                }
                (false, false) => {
                    return Err(ConfigurationError::DockerfileOrImage {
                        environment: env.0.clone(),
                    })
                }
                _ => (),
            }
        }
        Ok(envs)
    }

    fn expand_env_vars(vec: &mut Vec<String>) {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        vec.iter_mut()
            .for_each(|s| *s = envmnt::expand(&s, Some(options)));
    }
}
