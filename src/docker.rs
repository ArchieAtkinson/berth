use crate::presets::Env;
use bollard::{
    container::{ListContainersOptions, StartContainerOptions, StopContainerOptions},
    Docker,
};
use log::info;
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    process::{Command, Output},
};

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("Failed to conntect to docker daemon:\n {:?}", error)]
    ConnectingToDaemon { error: bollard::errors::Error },

    #[error("Failed to list containers:\n {:?}", error)]
    ListingContainers { error: bollard::errors::Error },

    #[error("Failed to remove container:\n {:?}", error)]
    RemovingContainer { error: bollard::errors::Error },

    #[error("Failed to start container:\n {:?}", error)]
    StartingContainer { error: bollard::errors::Error },

    #[error("Failed to stop container:\n {:?}", error)]
    StoppingContainer { error: bollard::errors::Error },

    #[error("Entering container failed: {reason}")]
    EnterExecFailure { reason: String },

    #[error("The following command return an error code:\n {cmd}\nWith:\n{stderr}")]
    CommandErrorCode { cmd: String, stderr: String },

    #[error("The following command failed:\n {cmd}\nDue to an unknown signal")]
    CommandKilled { cmd: String },

    #[error("The following command failed to run:\n {cmd}")]
    CommandFailed { cmd: String },
}

macro_rules! docker_err {
    ($variant:ident) => {
        |error| DockerError::$variant { error }
    };
}

const CONTAINER_PREFIX: &str = "berth-";
const CONTAINER_ENGINE: &str = "docker";

pub struct ContainerEngine {
    env: Env,
    no_tty: bool,
    docker: Docker,
}

impl ContainerEngine {
    pub fn new(mut env: Env, no_tty: bool) -> Result<Self, DockerError> {
        let mut hasher = DefaultHasher::new();
        env.hash(&mut hasher);
        env.name = format!("{}{}-{:016x}", CONTAINER_PREFIX, env.name, hasher.finish());
        Ok(ContainerEngine {
            env,
            no_tty,
            docker: Docker::connect_with_local_defaults()
                .map_err(docker_err!(ConnectingToDaemon))?,
        })
    }

    pub async fn create_new_environment(&self) -> Result<(), DockerError> {
        self.delete_container_if_exists().await?;
        self.create_container()?;
        self.start_container().await?;
        self.exec_setup_commands()
    }

    pub async fn enter_environment(&self) -> Result<(), DockerError> {
        let mut enter_args = vec!["exec"];
        if !self.no_tty {
            enter_args.push("-it");
        }

        if let Some(user) = &self.env.user {
            enter_args.extend(["-u", user]);
        }

        if let Some(entry_dir) = &self.env.entry_dir {
            enter_args.extend(["-w", entry_dir]);
        }

        enter_args.push(&self.env.name);

        let init_cmd = shell_words::split(&self.env.init_cmd).unwrap();
        enter_args.extend_from_slice(&init_cmd.iter().map(|s| s.as_str()).collect::<Vec<&str>>());

        let command = format!("{CONTAINER_ENGINE} {}", shell_words::join(&enter_args));

        info!("{command}");

        let exit_code = Command::new(CONTAINER_ENGINE)
            .args(&enter_args)
            .status()
            .map_err(|_| DockerError::CommandFailed { cmd: command })?
            .code();

        let error_str = match exit_code {
            Some(0) => None,
            Some(125) => Some("Docker exec failed to run"),
            Some(126) => Some("Command cannot execute"),
            Some(127) => Some("Command not found"),
            Some(130) => None, // Interrupt from Ctrl+C
            Some(_) => None,
            None => Some("Container was exited by siginal"),
        };

        if let Some(error_str) = error_str {
            return Err(DockerError::EnterExecFailure {
                reason: error_str.to_string(),
            });
        }

        if !self.is_anyone_connected().await? {
            self.stop_container().await?;
        }

        Ok(())
    }

    pub async fn does_environment_exist(&self) -> Result<bool, DockerError> {
        let mut filters = HashMap::new();
        filters.insert("name", vec![self.env.name.as_str()]);
        let options = Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        });
        let container_list = self
            .docker
            .list_containers(options)
            .await
            .map_err(docker_err!(ListingContainers))?;

        Ok(!container_list.is_empty())
    }

    pub async fn delete_container_if_exists(&self) -> Result<(), DockerError> {
        if self.does_environment_exist().await? {
            self.docker
                .remove_container(&self.env.name, None)
                .await
                .map_err(docker_err!(StoppingContainer))?;
        }
        Ok(())
    }

    pub async fn start_container(&self) -> Result<(), DockerError> {
        self.docker
            .start_container(&self.env.name, None::<StartContainerOptions<String>>)
            .await
            .map_err(docker_err!(StartingContainer))?;
        Ok(())
    }

    fn create_container(&self) -> Result<(), DockerError> {
        let mut create_args = vec!["create", "-it", "--name", &self.env.name];

        for mount in self.env.mounts.iter().flatten() {
            create_args.extend_from_slice(&["-v", &mount]);
        }

        create_args.push(&self.env.image);
        Self::run_docker_command(create_args)
    }

    fn exec_setup_commands(&self) -> Result<(), DockerError> {
        if let Some(cmds) = &self.env.exec_cmds {
            for cmd in cmds {
                let exec_full_cmd = format!("{} {} {}", "exec", &self.env.name, cmd);
                let exec_args = shell_words::split(&exec_full_cmd).unwrap();
                let exec_args = exec_args.iter().map(|s| s.as_str()).collect();
                Self::run_docker_command(exec_args)?;
            }
        }

        Ok(())
    }

    pub async fn stop_container(&self) -> Result<(), DockerError> {
        self.docker
            .stop_container(&self.env.name, Some(StopContainerOptions { t: 0 }))
            .await
            .map_err(docker_err!(StoppingContainer))?;
        Ok(())
    }

    async fn is_anyone_connected(&self) -> Result<bool, DockerError> {
        let args = vec!["exec", &self.env.name, "ls", "/dev/pts"];
        let output = Self::run_docker_command_with_output(args)?;
        let ps_count = String::from_utf8(output.stdout).unwrap().lines().count();

        let no_connections_ps_count = 2;
        Ok(ps_count > no_connections_ps_count)
    }

    fn run_docker_command_with_output(args: Vec<&str>) -> Result<Output, DockerError> {
        let command = format!("{} {}", CONTAINER_ENGINE, shell_words::join(&args));
        info!("{command}");

        let output = Command::new(CONTAINER_ENGINE)
            .args(&args)
            .output()
            .map_err(|_| DockerError::CommandFailed {
                cmd: command.clone(),
            })?;

        let status_code = output.status.code();
        match status_code {
            None => {
                let err = DockerError::CommandKilled { cmd: command };
                return Err(err);
            }
            Some(0) => (),
            Some(_n) => {
                let err = DockerError::CommandErrorCode {
                    cmd: command,
                    stderr: String::from_utf8(output.stderr.clone()).unwrap(),
                };
                return Err(err);
            }
        }
        Ok(output)
    }

    fn run_docker_command(args: Vec<&str>) -> Result<(), DockerError> {
        Self::run_docker_command_with_output(args).map(|_| ())
    }
}
