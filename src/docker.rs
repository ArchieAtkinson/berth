use crate::configuration::TomlEnvironment;
use bollard::{
    container::{ListContainersOptions, StartContainerOptions, StopContainerOptions},
    secret::ContainerSummary,
    Docker,
};
use log::info;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::Read,
    process::{Command, Output},
};

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("Failed to connect to docker daemon:\n {:?}", error)]
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

const BERTH_PREFIX: &str = "berth-";
const CONTAINER_ENGINE: &str = "docker";

#[derive(Debug)]
pub struct DockerHandler {
    name: String,
    image: String,
    entry_cmd: String,
    entry_options: Vec<String>,
    exec_cmds: Vec<String>,
    exec_options: Vec<String>,
    create_options: Vec<String>,
    docker: Docker,
}

impl DockerHandler {
    pub fn new(environment: TomlEnvironment, name: &str) -> Result<Self, DockerError> {
        let docker =
            Docker::connect_with_local_defaults().map_err(docker_err!(ConnectingToDaemon))?;

        let image = {
            if !environment.dockerfile.is_empty() {
                Self::build_dockerfile(&environment.dockerfile, name)?
            } else {
                environment.image
            }
        };
        let mut handle = DockerHandler {
            name: name.to_string(),
            image,
            entry_cmd: environment.entry_cmd,
            entry_options: environment.entry_options,
            exec_cmds: environment.exec_cmds,
            exec_options: environment.exec_options,
            create_options: environment.create_options,
            docker,
        };

        let mut hasher = DefaultHasher::new();
        handle.hash(&mut hasher);

        handle.name = format!("{}{}-{:016x}", BERTH_PREFIX, name, hasher.finish());

        Ok(handle)
    }

    pub fn build_dockerfile(dockerfile: &str, name: &str) -> Result<String, DockerError> {
        let path = fs::canonicalize(dockerfile).unwrap();
        if !path.exists() || !path.is_file() {
            panic!("BAD FILE");
        }

        let mut file = File::open(&path).unwrap();
        let mut hasher = Sha256::new();
        let mut buffer = [0; 1024];

        loop {
            let bytes_read = file.read(&mut buffer).unwrap();
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let image_name = format!(
            "{}{}-{:016x}",
            BERTH_PREFIX,
            name.to_lowercase(),
            hasher.finalize()
        );
        let args = vec![
            "build",
            "-t",
            &image_name,
            "-f",
            &path.to_str().unwrap(),
            ".",
        ];
        Self::run_docker_command(args)?;

        Ok(image_name)
    }

    pub async fn create_new_environment(&self) -> Result<(), DockerError> {
        self.delete_container_if_exists().await?;
        self.create_container()?;
        self.start_container().await?;
        self.exec_setup_commands()
    }

    fn to_shell(strings: &Vec<String>) -> Vec<String> {
        strings
            .iter()
            .flat_map(|s| shell_words::split(s).unwrap())
            .collect()
    }

    pub async fn enter_environment(&self) -> Result<(), DockerError> {
        let mut args = vec!["exec"];

        let options = Self::to_shell(&self.entry_options);
        args.extend(options.iter().map(|s| s.as_str()));

        args.push(&self.name);

        let init_cmd = shell_words::split(&self.entry_cmd).unwrap();
        args.extend_from_slice(&init_cmd.iter().map(|s| s.as_str()).collect::<Vec<&str>>());

        let command = format!("{CONTAINER_ENGINE} {}", shell_words::join(&args));

        info!("{command}");

        let exit_code = Command::new(CONTAINER_ENGINE)
            .args(&args)
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
            None => Some("Container was exited by signal"),
        };

        if let Some(error_str) = error_str {
            return Err(DockerError::EnterExecFailure {
                reason: error_str.to_string(),
            });
        }

        if self.is_container_running().await? && !self.is_anyone_connected().await? {
            self.stop_container().await?;
        }

        Ok(())
    }

    pub async fn get_container_info(&self) -> Result<Option<ContainerSummary>, DockerError> {
        let mut filters = HashMap::new();
        filters.insert("name", vec![self.name.as_str()]);
        let options = Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        });

        let mut container_list = self
            .docker
            .list_containers(options)
            .await
            .map_err(docker_err!(ListingContainers))?;

        Ok(container_list.pop())
    }

    pub async fn is_container_running(&self) -> Result<bool, DockerError> {
        Ok(self
            .get_container_info()
            .await?
            .map_or(false, |c| c.state == Some("running".to_string())))
    }

    pub async fn does_environment_exist(&self) -> Result<bool, DockerError> {
        Ok(self.get_container_info().await?.is_some())
    }

    pub async fn delete_container_if_exists(&self) -> Result<(), DockerError> {
        if self.does_environment_exist().await? {
            self.docker
                .remove_container(&self.name, None)
                .await
                .map_err(docker_err!(StoppingContainer))?;
        }
        Ok(())
    }

    pub async fn start_container(&self) -> Result<(), DockerError> {
        self.docker
            .start_container(&self.name, None::<StartContainerOptions<String>>)
            .await
            .map_err(docker_err!(StartingContainer))?;
        Ok(())
    }

    fn create_container(&self) -> Result<(), DockerError> {
        let mut args = vec!["create", "--name", &self.name];

        let options = Self::to_shell(&self.create_options);
        args.extend(options.iter().map(|s| s.as_str()));

        args.push(&self.image);
        args.extend_from_slice(&["tail", "-f", "/dev/null"]);
        Self::run_docker_command(args)
    }

    fn exec_setup_commands(&self) -> Result<(), DockerError> {
        for cmd in &self.exec_cmds {
            let mut args = vec!["exec"];

            let options = Self::to_shell(&self.exec_options);
            args.extend(options.iter().map(|s| s.as_str()));

            args.push(&self.name);

            let split_cmd = shell_words::split(cmd).unwrap();
            args.extend(split_cmd.iter().map(|s| s.as_str()));

            Self::run_docker_command(args)?;
        }
        Ok(())
    }

    pub async fn stop_container(&self) -> Result<(), DockerError> {
        self.docker
            .stop_container(&self.name, Some(StopContainerOptions { t: 0 }))
            .await
            .map_err(docker_err!(StoppingContainer))?;
        Ok(())
    }

    pub async fn is_anyone_connected(&self) -> Result<bool, DockerError> {
        let args = vec!["exec", &self.name, "ls", "/dev/pts"];
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

impl Hash for DockerHandler {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.image.hash(state);
        self.entry_cmd.hash(state);
        self.entry_options.hash(state);
        self.exec_cmds.hash(state);
        self.exec_options.hash(state);
        self.create_options.hash(state);
    }
}
