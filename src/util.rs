use std::{collections::HashMap, path::PathBuf};

#[derive(Debug)]
pub struct EnvVar {
    pub home: Option<PathBuf>,
    pub xdg_config_path: Option<PathBuf>,
}

impl EnvVar {
    pub fn new<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let vars: HashMap<_, _> = iter.into_iter().collect();

        EnvVar {
            home: Self::get_path(&vars, "HOME"),
            xdg_config_path: Self::get_path(&vars, "XDG_CONFIG_PATH"),
        }
    }

    fn get_path(vars: &HashMap<String, String>, key: &str) -> Option<PathBuf> {
        vars.get(key).map(PathBuf::from)
    }
}
