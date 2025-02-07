use clap::Parser;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{clap_error}")]
    BadInput { clap_error: String },

    #[error("Could not find file at 'config-path': {path:?}")]
    NoConfigAtProvidedPath { path: OsString },

    #[error("Could not find config file in $XDG_CONFIG_PATH or $HOME")]
    NoConfigInStandardLocation,
}

#[derive(Parser, Debug)]
#[command(
    about = "berth is a CLI that lets you let you create development environments without touching repository code"
)]
struct Cli {
    /// Path to config file
    #[arg(long, value_name = "FILE")]
    pub config_path: Option<PathBuf>,

    /// Deletes container on exit
    #[arg(long, default_value_t = false)]
    pub cleanup: bool,

    /// The environment from your config file to use
    pub env_name: String,
}

pub struct AppConfig {
    pub config_path: PathBuf,
    pub env_name: String,
    pub cleanup: bool,
}

impl AppConfig {
    pub fn new<I, T>(args: I) -> Result<AppConfig, CliError>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let cli = match Cli::try_parse_from(args) {
            Ok(v) => v,
            Err(e) => {
                return Err(CliError::BadInput {
                    clap_error: e.to_string(),
                })
            }
        };

        Ok(AppConfig {
            config_path: Self::set_config_path(cli.config_path)?,
            env_name: cli.env_name,
            cleanup: cli.cleanup,
        })
    }

    fn set_config_path(config_path: Option<PathBuf>) -> Result<PathBuf, CliError> {
        if let Some(path) = config_path {
            return if path.exists() && path.is_file() {
                Ok(path)
            } else {
                Err(CliError::NoConfigAtProvidedPath {
                    path: path.as_os_str().into(),
                })
            };
        }

        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_PATH") {
            let xdg_path = Path::new(&xdg_config)
                .join(".config")
                .join("berth")
                .join("config.toml");
            if xdg_path.exists() {
                return Ok(xdg_path);
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let home_path = Path::new(&home)
                .join(".config")
                .join("berth")
                .join("config.toml");
            if home_path.exists() {
                return Ok(home_path);
            }
        }

        Err(CliError::NoConfigInStandardLocation)
    }
}
