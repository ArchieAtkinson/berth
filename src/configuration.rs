use std::collections::HashMap;

use envmnt::{ExpandOptions, ExpansionType};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("{message}")]
    TomlParse { message: String },
}

#[derive(Debug, Deserialize, Hash)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    #[serde(skip_deserializing)]
    pub name: String,
    pub image: String,
    pub entry_cmd: String,

    pub entry_options: Option<Vec<String>>,

    pub exec_cmds: Option<Vec<String>>,
    pub exec_options: Option<Vec<String>>,

    pub create_options: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    #[serde(rename = "env")]
    pub environments: HashMap<String, Environment>,
}

impl Configuration {
    pub fn new(file: &str) -> Result<Configuration, ConfigurationError> {
        match toml::from_str::<Configuration>(file) {
            Ok(v) => Ok(Configuration {
                environments: Self::parse_envs(v.environments),
            }),
            Err(e) => Err(ConfigurationError::TomlParse {
                message: e.to_string(),
            }),
        }
    }

    fn parse_envs(envs: HashMap<String, Environment>) -> HashMap<String, Environment> {
        envs.into_iter()
            .map(|(name, mut env)| {
                env.name = name.clone();
                env.entry_options = env.entry_options.map(|s| Self::expand_env_vars(s));
                env.exec_options = env.exec_options.map(|s| Self::expand_env_vars(s));
                env.create_options = env.create_options.map(|s| Self::expand_env_vars(s));
                (name, env)
            })
            .collect()
    }

    fn expand_env_vars(vec: Vec<String>) -> Vec<String> {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        vec.into_iter()
            .map(|mount| envmnt::expand(&mount, Some(options)).to_string())
            .collect()
    }
}
