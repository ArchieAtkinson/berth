use clap::Parser;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::util::EnvVar;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(long, value_name = "FILE")]
    pub config_path: Option<PathBuf>,

    pub env_name: String,
}

pub struct AppConfig {
    pub config_path: PathBuf,
    pub env_name: String,
}

impl AppConfig {
    pub fn new<I, T>(args: I, env_vars: &EnvVar) -> Result<AppConfig, String>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let cli = match Cli::try_parse_from(args) {
            Ok(v) => v,
            Err(e) => return Err(e.to_string()),
        };
        Ok(AppConfig {
            config_path: Self::set_config_path(cli.config_path, env_vars)?,
            env_name: cli.env_name,
        })
    }

    fn set_config_path(config_path: Option<PathBuf>, env_vars: &EnvVar) -> Result<PathBuf, String> {
        if let Some(path) = config_path {
            return if path.exists() {
                Ok(path)
            } else {
                Err(format!(
                    "Could not find config file at provided path: {:?}",
                    path.as_os_str()
                ))
            };
        }

        if let Some(xdg_config) = &env_vars.xdg_config_path {
            let xdg_path = Path::new(&xdg_config)
                .join(".config")
                .join("berth")
                .join("config.toml");
            if xdg_path.exists() {
                return Ok(xdg_path);
            }
        }

        if let Some(home) = &env_vars.home {
            let home_path = Path::new(&home)
                .join(".config")
                .join("berth")
                .join("config.toml");
            if home_path.exists() {
                return Ok(home_path);
            }
        }

        Err("Could not find config file in $XDG_CONFIG_PATH or $HOME".to_string())
    }
}
