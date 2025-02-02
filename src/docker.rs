use crate::presets::Env;
use log::info;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    process::{Command, Output},
};

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("The following command return an error code:\n {cmd}\nWith:\n{stderr}")]
    CommandErrorCode { cmd: String, stderr: String },

    #[error("The following command failed:\n {cmd}\nDue to an unknown signal")]
    CommandKilled { cmd: String },

    #[error("The following command failed to run:\n {cmd}")]
    CommandFailed { cmd: String },
}

const CONTAINER_PREFIX: &str = "berth-";
const CONTAINER_ENGINE: &str = "docker";

pub struct Docker {
    env: Env,
    no_tty: bool,
}

impl Docker {
    pub fn new(mut env: Env, no_tty: bool) -> Self {
        let mut hasher = DefaultHasher::new();
        env.hash(&mut hasher);
        env.name = format!("{}{}-{:016x}", CONTAINER_PREFIX, env.name, hasher.finish());
        Docker { env, no_tty }
    }

    pub fn create_new_environment(&self) -> Result<(), DockerError> {
        self.delete_container_if_exists()?;
        self.create_container()?;
        self.start_container()?;
        self.exec_setup_commands()
    }

    pub fn enter_environment(&self) -> Result<(), DockerError> {
        let mut enter_args = vec!["exec"];
        if !self.no_tty {
            enter_args.push("-it");
        }

        if let Some(user) = &self.env.user {
            enter_args.extend_from_slice(&["-u", user]);
        }

        if let Some(entry_dir) = &self.env.entry_dir {
            enter_args.extend_from_slice(&["-w", entry_dir]);
        }

        enter_args.extend_from_slice(&[&self.env.name, &self.env.init_cmd]);

        let command = format!("{CONTAINER_ENGINE} {}", shell_words::join(&enter_args));
        info!("{command}");

        Command::new(CONTAINER_ENGINE)
            .args(&enter_args)
            .status()
            .map_err(|_| DockerError::CommandFailed { cmd: command })?;

        if !self.is_anyone_connected()? {
            self.stop_container()?;
        }
        Ok(())
    }

    pub fn does_environment_exist(&self) -> Result<bool, DockerError> {
        let filter = format!("name={}", &self.env.name);
        let ls_args = vec!["container", "ls", "-a", "--quiet", "--filter", &filter];

        let ls_output = Self::run_docker_command_with_output(ls_args)?;

        Ok(!ls_output.stdout.is_empty())
    }

    pub fn delete_container_if_exists(&self) -> Result<(), DockerError> {
        if self.does_environment_exist()? {
            let rm_args = vec!["container", "rm", &self.env.name];
            Self::run_docker_command(rm_args)?;
        }
        Ok(())
    }

    pub fn start_container(&self) -> Result<(), DockerError> {
        let start_args = vec!["start", &self.env.name];
        Self::run_docker_command(start_args)
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

    fn stop_container(&self) -> Result<(), DockerError> {
        Self::run_docker_command(vec!["stop", "-t", "0", &self.env.name])
    }

    fn is_anyone_connected(&self) -> Result<bool, DockerError> {
        let args = vec!["exec", &self.env.name, "ls", "/dev/pts"];
        let output = Self::run_docker_command_with_output(args)?;
        let ps_count = String::from_utf8(output.stdout).unwrap().lines().count();

        let no_connections_ps_count = 2;
        Ok(ps_count > no_connections_ps_count)
    }
}
