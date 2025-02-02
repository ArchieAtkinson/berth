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
    pub name: String, // Get this from the Preset

    pub image: String,
    pub exec_cmds: Option<Vec<String>>,
    pub mounts: Option<Vec<String>>,
    pub init_cmd: String,
    pub user: Option<String>,
    pub entry_dir: Option<String>,
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
                env.mounts = env.mounts.map(|mounts| Self::expand_env_vars(mounts));

                (name, env)
            })
            .collect()
    }

    fn expand_env_vars(mounts: Vec<String>) -> Vec<String> {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        mounts
            .into_iter()
            .map(|mount| envmnt::expand(&mount, Some(options)).to_string())
            .collect()
    }
}
