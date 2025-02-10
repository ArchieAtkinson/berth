use thiserror::Error;

use crate::{cli::CliError, configuration::ConfigError, docker::DockerError};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Docker(#[from] DockerError),

    #[error(transparent)]
    Cli(#[from] CliError),

    #[error(transparent)]
    Configuration(#[from] ConfigError),
}
