use crate::{configuration::Environment, util::Spinner, UnexpectedExt};
use bollard::{
    container::{ListContainersOptions, StartContainerOptions, StopContainerOptions},
    image::ListImagesOptions,
    secret::ContainerSummary,
    Docker,
};
use log::info;
use miette::{Diagnostic, Result};
use std::{
    collections::HashMap,
    process::{Command, Output},
};

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum DockerError {
    #[error("Failed to connect to docker daemon with the following error:\n{0:?}\n")]
    #[diagnostic(code(cli::daemon), help("Is the Docker daemon running?"))]
    ConnectingToDaemon(bollard::errors::Error),

    #[error("Failed to get container information with the following error:\n{0}\n")]
    #[diagnostic(code(cli::container::info), help("Is the Docker daemon running?"))]
    ContainerInfo(bollard::errors::Error),

    #[error("Failed to get image information with the following error:\n{0}\n")]
    #[diagnostic(code(cli::image::info), help("Is the Docker daemon running?"))]
    ImageInfo(bollard::errors::Error),

    #[error("Failed to remove container with the following error:\n{0}\n")]
    #[diagnostic(code(cli::container::removing), help("Is the Docker daemon running?"))]
    RemovingContainer(bollard::errors::Error),

    #[error("Failed to start container with the following error:\n{0}\n")]
    #[diagnostic(code(cli::container::starting), help("Is the Docker daemon running?"))]
    StartingContainer(bollard::errors::Error),

    #[error("Failed to stop container with the following error:\n{0}\n")]
    #[diagnostic(code(cli::container::stopping), help("Is the Docker daemon running?"))]
    StoppingContainer(bollard::errors::Error),

    #[error("Entering container failed with the following error:\n{0}\n")]
    #[diagnostic(code(cli::container::entering))]
    EnteringContainer(String),

    #[error("The following command return an error code:\n\n{cmd}\n\n")]
    #[diagnostic(code(cli::container::command::exitcode), help("{stderr}"))]
    CommandExitCode { cmd: String, stderr: String },

    #[error("The following command failed due to an unknown signal:\n{0}")]
    #[diagnostic(code(cli::container::command::killed))]
    CommandKilled(String),

    #[error("The following command failed to run:\n{0}")]
    #[diagnostic(code(cli::container::command::failed))]
    CommandFailed(String),
}

macro_rules! docker_err {
    ($variant:ident) => {
        |error| DockerError::$variant(error)
    };
}

const CONTAINER_ENGINE: &str = "docker";

#[derive(Debug)]
pub struct DockerHandler {
    env: Environment,
    docker: Docker,
}

impl DockerHandler {
    pub fn new(environment: Environment) -> Result<Self> {
        let docker =
            Docker::connect_with_local_defaults().map_err(docker_err!(ConnectingToDaemon))?;

        Ok(DockerHandler {
            env: environment,
            docker,
        })
    }

    async fn does_image_need_building(&self) -> Result<bool> {
        if self.env.dockerfile.is_some() {
            let mut filters = HashMap::new();
            filters.insert("reference", vec![self.env.image.as_str()]);
            let options = Some(ListImagesOptions {
                all: false,
                filters,
                digests: false,
            });

            let out = self
                .docker
                .list_images(options)
                .await
                .map_err(docker_err!(ImageInfo))?;

            return Ok(out.is_empty());
        }
        Ok(false)
    }

    fn build_image_from_dockerfile(&self) -> Result<()> {
        let spinner = Spinner::new("Building Dockerfile");

        let dockerfile_path = self
            .env
            .dockerfile
            .as_ref()
            .unexpected()?
            .as_path()
            .to_string_lossy()
            .to_string();
        let args = vec!["build", "-t", &self.env.image, "-f", &dockerfile_path, "."];
        Self::run_docker_command(args)?;

        spinner.finish_and_clear();

        Ok(())
    }

    pub async fn create_new_environment(&self) -> Result<()> {
        if self.does_image_need_building().await? {
            self.build_image_from_dockerfile()?;
        }

        self.delete_container_if_exists().await?;

        let spinner = Spinner::new("Creating Container");

        self.create_container()?;
        self.start_container().await?;
        self.exec_setup_commands()?;

        spinner.finish_and_clear();
        Ok(())
    }

    fn to_shell(strings: &[String]) -> Vec<String> {
        strings
            .iter()
            .flat_map(|s| shell_words::split(s).unwrap())
            .collect()
    }

    pub async fn enter_environment(&self) -> Result<()> {
        let mut args = vec!["exec"];

        let options = Self::to_shell(&self.env.entry_options);
        args.extend(options.iter().map(|s| s.as_str()));

        args.push(&self.env.name);

        let init_cmd = shell_words::split(&self.env.entry_cmd).unwrap();
        args.extend_from_slice(&init_cmd.iter().map(|s| s.as_str()).collect::<Vec<&str>>());

        let command = format!("{CONTAINER_ENGINE} {}", shell_words::join(&args));

        info!("{command}");

        let exit_code = Command::new(CONTAINER_ENGINE)
            .args(&args)
            .status()
            .map_err(|_| DockerError::CommandFailed(command))?
            .code();

        let error_str = match exit_code {
            Some(0) => None,
            Some(125) => Some("Docker exec failed to run"),
            Some(126) => Some("Command cannot execute"),
            Some(127) => Some("Command not found"),
            Some(130) => None, // Interrupt from Ctrl+C
            Some(_) => None,
            None => Some("Container was exited by signal"),
        };

        if let Some(error_str) = error_str {
            return Err(DockerError::EnteringContainer(error_str.to_string()).into());
        }

        if !self.is_anyone_connected().await? {
            self.stop_container_if_running().await?;
        }

        Ok(())
    }

    pub async fn get_container_info(&self) -> Result<Option<ContainerSummary>> {
        let mut filters = HashMap::new();
        filters.insert("name", vec![self.env.name.as_str()]);
        let options = Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        });

        let mut container_list = self
            .docker
            .list_containers(options)
            .await
            .map_err(docker_err!(ContainerInfo))?;

        Ok(container_list.pop())
    }

    pub async fn is_container_running(&self) -> Result<bool> {
        Ok(self
            .get_container_info()
            .await?
            .is_some_and(|c| c.state == Some("running".to_string())))
    }

    pub async fn does_environment_exist(&self) -> Result<bool> {
        Ok(self.get_container_info().await?.is_some())
    }

    pub async fn delete_container_if_exists(&self) -> Result<()> {
        if self.does_environment_exist().await? {
            self.docker
                .remove_container(&self.env.name, None)
                .await
                .map_err(docker_err!(StoppingContainer))?;
        }
        Ok(())
    }

    pub async fn start_container(&self) -> Result<()> {
        self.docker
            .start_container(&self.env.name, None::<StartContainerOptions<String>>)
            .await
            .map_err(docker_err!(StartingContainer))?;
        Ok(())
    }

    fn create_container(&self) -> Result<()> {
        let mut args = vec!["create", "--name", &self.env.name];

        let options = Self::to_shell(&self.env.create_options);
        args.extend(options.iter().map(|s| s.as_str()));

        args.push(&self.env.image);
        args.extend_from_slice(&["tail", "-f", "/dev/null"]);
        Self::run_docker_command(args)
    }

    fn exec_setup_commands(&self) -> Result<()> {
        for cmd in &self.env.exec_cmds {
            let mut args = vec!["exec"];

            let options = Self::to_shell(&self.env.exec_options);
            args.extend(options.iter().map(|s| s.as_str()));

            args.push(&self.env.name);

            let split_cmd = shell_words::split(cmd).unwrap();
            args.extend(split_cmd.iter().map(|s| s.as_str()));

            Self::run_docker_command(args)?;
        }
        Ok(())
    }

    pub async fn stop_container_if_running(&self) -> Result<()> {
        if self.is_container_running().await? {
            self.docker
                .stop_container(&self.env.name, Some(StopContainerOptions { t: 0 }))
                .await
                .map_err(docker_err!(StoppingContainer))?;
        }
        Ok(())
    }

    pub async fn is_anyone_connected(&self) -> Result<bool> {
        let args = vec!["exec", &self.env.name, "ls", "/dev/pts"];
        let output = Self::run_docker_command_with_output(args)?;
        let ps_count = String::from_utf8(output.stdout).unwrap().lines().count();

        let no_connections_ps_count = 2;
        Ok(ps_count > no_connections_ps_count)
    }

    fn run_docker_command_with_output(args: Vec<&str>) -> Result<Output> {
        let command = format!("{} {}", CONTAINER_ENGINE, shell_words::join(&args));
        info!("{command}");

        let output = Command::new(CONTAINER_ENGINE)
            .args(&args)
            .output()
            .map_err(|_| DockerError::CommandFailed(command.clone()))?;

        let status_code = output.status.code();
        match status_code {
            None => Err(DockerError::CommandKilled(command).into()),
            Some(0) => Ok(output),
            Some(_) => Err(DockerError::CommandExitCode {
                cmd: command,
                stderr: String::from_utf8(output.stderr.clone()).unwrap(),
            }
            .into()),
        }
    }

    fn run_docker_command(args: Vec<&str>) -> Result<()> {
        Self::run_docker_command_with_output(args).map(|_| ())
    }
}
