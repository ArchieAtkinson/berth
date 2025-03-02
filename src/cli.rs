use clap::Parser;
use miette::{Diagnostic, Result};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum CliError {
    #[error("{0}")]
    BadInput(String),

    #[error("Could not find file at 'config-path': {0:?}")]
    NoConfigAtProvidedPath(OsString),

    #[error("Could not find config file in $XDG_CONFIG_HOME or $HOME")]
    NoConfigInStandardLocation,
}

#[derive(Parser, Debug)]
#[command(
    about = "berth, A CLI to help create development environments without touching repository code",
    trailing_var_arg = false
)]
struct Cli {
    /// Path to config file
    #[arg(long, value_name = "FILE")]
    pub config_path: Option<PathBuf>,

    /// Deletes container on exit
    #[arg(long, default_value_t = false)]
    pub cleanup: bool,

    /// Build/rebuild the environment instead of starting it
    #[arg(long, default_value_t = false, group = "action")]
    pub build: bool,

    /// View environment definition after it has been parsed by berth
    #[arg(long, default_value_t = false, group = "action")]
    pub view: bool,

    /// The environment to be used
    pub environment: String,
}

#[derive(Clone)]
pub enum Action {
    Up,
    Build,
    View,
}

#[derive(Clone)]
pub struct AppConfig {
    pub config_path: PathBuf,
    pub action: Action,
    pub cleanup: bool,
    pub environment: String,
}

impl AppConfig {
    pub fn new<I, T>(args: I) -> Result<AppConfig>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let cli = match Cli::try_parse_from(args) {
            Ok(v) => v,
            Err(e) => {
                if e.kind() == clap::error::ErrorKind::DisplayHelp
                    || e.kind() == clap::error::ErrorKind::DisplayVersion
                {
                    println!("{}", e);
                    std::process::exit(0);
                } else {
                    return Err(CliError::BadInput(e.to_string()).into());
                }
            }
        };

        let action = match (cli.view, cli.build) {
            (true, false) => Action::View,
            (false, true) => Action::Build,
            (false, false) => Action::Up,
            (true, true) => panic!("Parsing should catch this"),
        };

        Ok(AppConfig {
            config_path: Self::set_config_path(cli.config_path)?,
            action,
            cleanup: cli.cleanup,
            environment: cli.environment,
        })
    }

    fn set_config_path(config_path: Option<PathBuf>) -> Result<PathBuf> {
        if let Some(path) = config_path {
            return if path.exists() && path.is_file() {
                Ok(path)
            } else {
                Err(CliError::NoConfigAtProvidedPath(path.as_os_str().into()).into())
            };
        }

        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
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

        Err(CliError::NoConfigInStandardLocation.into())
    }
}
