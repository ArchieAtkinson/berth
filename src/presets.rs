use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Env {
    pub name: String,
    pub image: String,
    pub exec_cmds: Option<Vec<String>>,
    pub mounts: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub env: Vec<Env>,
}

impl Preset {
    pub fn new(file: &str) -> Result<Preset, toml::de::Error> {
        toml::from_str::<Preset>(file)
    }
}
