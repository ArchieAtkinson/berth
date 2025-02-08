use thiserror::Error;

use crate::{cli::CliError, docker::DockerError, configuration::ConfigurationError};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Docker(#[from] DockerError),

    #[error(transparent)]
    Cli(#[from] CliError),

    #[error(transparent)]
    Configuration(#[from] ConfigurationError),

    #[error("Proved environment, '{name}' is not in loaded config")]
    ProvidedEnvNameNotInConfig { name: String },
}
