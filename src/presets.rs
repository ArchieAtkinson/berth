use std::collections::HashMap;

use envmnt::{ExpandOptions, ExpansionType};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PresetError {
    #[error("{message}")]
    TomlParse { message: String },
}

#[derive(Debug, Deserialize, Hash)]
#[serde(deny_unknown_fields)]
pub struct Env {
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
pub struct Preset {
    #[serde(rename = "env")]
    pub envs: HashMap<String, Env>,
}

impl Preset {
    pub fn new(file: &str) -> Result<Preset, PresetError> {
        match toml::from_str::<Preset>(file) {
            Ok(v) => Ok(Preset {
                envs: Self::parse_envs(v.envs),
            }),
            Err(e) => Err(PresetError::TomlParse {
                message: e.to_string(),
            }),
        }
    }

    fn parse_envs(envs: HashMap<String, Env>) -> HashMap<String, Env> {
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
