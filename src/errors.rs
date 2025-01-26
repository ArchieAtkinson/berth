use thiserror::Error;

use crate::{cli::CliError, docker::DockerError, presets::PresetError};

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Docker(#[from] DockerError),

    #[error(transparent)]
    Cli(#[from] CliError),

    #[error(transparent)]
    Preset(#[from] PresetError),

    #[error("Proved Enviroment, '{name}' is not in loaded config")]
    ProvidedEnvNameNotInConfig { name: String },
}
