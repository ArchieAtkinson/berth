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

    #[serde(default)]
    pub entry_options: Vec<String>,

    #[serde(default)]
    pub exec_cmds: Vec<String>,

    #[serde(default)]
    pub exec_options: Vec<String>,

    #[serde(default)]
    pub create_options: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    #[serde(rename = "environment")]
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
                Self::expand_env_vars(&mut env.entry_options);
                Self::expand_env_vars(&mut env.exec_options);
                Self::expand_env_vars(&mut env.create_options);
                (name, env)
            })
            .collect()
    }

    fn expand_env_vars(vec: &mut Vec<String>) {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        vec.iter_mut()
            .for_each(|s| *s = envmnt::expand(&s, Some(options)));
    }
}
