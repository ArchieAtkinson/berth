use std::collections::HashMap;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PresetError {
    #[error("{message}")]
    TomlParse { message: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Env {
    #[serde(skip_deserializing)]
    pub name: String, // Get this from the Preset

    pub image: String,
    pub exec_cmds: Option<Vec<String>>,
    pub mounts: Option<Vec<String>>,
    pub init_cmd: String,
    pub user: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub env: HashMap<String, Env>,
}

impl Preset {
    pub fn new(file: &str) -> Result<Preset, PresetError> {
        match toml::from_str::<Preset>(file) {
            Ok(v) => Ok(Preset {
                env: v
                    .env
                    .into_iter()
                    .map(|(key, mut env)| {
                        env.name = key.clone();
                        (key, env)
                    })
                    .collect(),
            }),
            Err(e) => Err(PresetError::TomlParse {
                message: e.to_string(),
            }),
        }
    }
}
