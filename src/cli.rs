use clap::{Parser, Subcommand};
use miette::{Diagnostic, Result};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::env::Env;

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
    about = "berth, A CLI to help add you dev environment into a devcontainer without touching repository code",
    trailing_var_arg = false
)]
struct Cli {
    #[arg(long, value_name = "FILE")]
    pub config_path: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub cleanup: bool,

    #[command(subcommand)]
    pub action: Action,
}

#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum Action {
    Init { environment: String },
    Up,
    Build,
    View,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub config_path: PathBuf,
    pub action: Action,
    pub cleanup: bool,
}

impl Settings {
    pub fn new<I, T>(args: I, env: &impl Env) -> Result<Settings>
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

        Ok(Settings {
            config_path: Self::set_config_path(cli.config_path, env)?,
            action: cli.action,
            cleanup: cli.cleanup,
        })
    }

    fn set_config_path(config_path: Option<PathBuf>, env: &impl Env) -> Result<PathBuf> {
        if let Some(path) = config_path {
            return if path.exists() && path.is_file() {
                Ok(path)
            } else {
                Err(CliError::NoConfigAtProvidedPath(path.as_os_str().into()).into())
            };
        }

        let rel_config_path = Path::new(".config/berth/config.toml");

        ["XDG_CONFIG_HOME", "HOME"]
            .iter()
            .filter_map(|&v| env.var(v))
            .map(|v| PathBuf::from(v).join(rel_config_path))
            .find(|p| p.exists() && p.is_file())
            .ok_or_else(|| CliError::NoConfigInStandardLocation.into())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use insta::assert_snapshot;
    use tempfile::{NamedTempFile, TempDir};

    use crate::env::tests::MockEnv;

    use super::*;

    #[test]
    fn no_command() {
        let env = MockEnv::new();
        let config = Settings::new(["berth"], &env);

        let err = config.err().unwrap();
        assert_snapshot!(err);
    }

    #[test]
    fn no_config() {
        let env = MockEnv::new();
        let config = Settings::new(["berth", "up"], &env);

        let err = config.err().unwrap();
        assert_snapshot!(err.to_string());
    }

    #[test]
    fn config_from_from_env_var() {
        let tmp_dir = TempDir::new().unwrap();
        let dir_path = tmp_dir.path().join("home");
        let file_path = dir_path.join(".config/berth/config.toml");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::File::create(&file_path).unwrap();

        let mut env = MockEnv::new();
        env.insert("HOME", dir_path.as_path().to_str().unwrap());

        let config = Settings::new(["berth", "up"], &env).unwrap();

        let config = config;
        let expected = Settings {
            config_path: file_path,
            action: Action::Up,
            cleanup: false,
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn up_command_from_config_path() {
        let tmp_file = NamedTempFile::new().unwrap();
        let file_path = tmp_file.path().to_str().unwrap();

        let env = MockEnv::new();
        let config = Settings::new(["berth", "--config-path", file_path, "up"], &env).unwrap();

        let config = config;
        let expected = Settings {
            config_path: file_path.into(),
            action: Action::Up,
            cleanup: false,
        };
        assert_eq!(config, expected);
    }
}
