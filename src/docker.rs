use crate::presets::Env;
use log::info;
use std::{
    env,
    process::{Command, Output},
};

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("The following command failed:\n {cmd}\nWith:\n{stderr}")]
    CommandFailed { cmd: String, stderr: String },

    #[error("The following command failed:\n {cmd}\nDue to an unknown signal")]
    CommandKilled { cmd: String },
}

pub const ENVIROMENT_PREFIX: &str = "berth-";

pub struct Docker {
    env: Env,
}

impl Docker {
    pub fn new(mut env: Env) -> Self {
        env.name = format!("{}{}", ENVIROMENT_PREFIX, env.name);
        if env.mount_working_dir {
            let local_working_dir = env::current_dir().unwrap();
            let env_working_dir = format!(
                "/berth/{}",
                local_working_dir.file_name().unwrap().to_str().unwrap()
            );
            let working_dir_mount = format!(
                "{}:{}",
                local_working_dir.display().to_string(),
                env_working_dir
            );
            info!("Adding mount: {}", &working_dir_mount);
            env.mounts
                .get_or_insert_with(Vec::new)
                .push(working_dir_mount);
            env.entry_dir = Some(env_working_dir.to_string());
        }
        Docker { env }
    }

    pub fn create_new_enviroment(&self) -> Result<(), DockerError> {
        self.delete_container_if_exists()?;
        self.create_container()?;
        self.start_container()?;
        self.exec_setup_commands()
    }

    pub fn enter_enviroment(&self) -> Result<(), DockerError> {
        let mut enter_args = vec!["exec", "-it"];

        if let Some(user) = &self.env.user {
            enter_args.extend_from_slice(&["-u", user]);
        }

        if let Some(entry_dir) = &self.env.entry_dir {
            enter_args.extend_from_slice(&["-w", entry_dir]);
        }

        enter_args.extend_from_slice(&[&self.env.name, &self.env.init_cmd]);

        info!("docker {}", shell_words::join(&enter_args));

        Command::new("docker").args(&enter_args).status().unwrap();

        self.stop_container()
    }

    pub fn does_enviroment_exist(&self) -> Result<bool, DockerError> {
        let filter = format!("name={}", &self.env.name);
        let ls_args = vec!["container", "ls", "-a", "--quiet", "--filter", &filter];

        let ls_output = Self::run_docker_command_with_output(ls_args)?;

        Ok(!ls_output.stdout.is_empty())
    }

    fn create_container(&self) -> Result<(), DockerError> {
        let mut create_args = vec!["create", "-it", "--name", &self.env.name];

        for mount in self.env.mounts.iter().flatten() {
            create_args.extend_from_slice(&["-v", &mount]);
        }

        create_args.push(&self.env.image);
        Self::run_docker_command(create_args)
    }

    fn start_container(&self) -> Result<(), DockerError> {
        let start_args = vec!["start", &self.env.name];
        Self::run_docker_command(start_args)
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

    fn delete_container_if_exists(&self) -> Result<(), DockerError> {
        if self.does_enviroment_exist()? {
            let rm_args = vec!["container", "rm", &self.env.name];
            Self::run_docker_command(rm_args)?;
        }
        Ok(())
    }

    fn run_docker_command_with_output(args: Vec<&str>) -> Result<Output, DockerError> {
        let container_engine_command = "docker";
        let output = Command::new(container_engine_command)
            .args(&args)
            .output()
            .unwrap();

        info!("{}", String::from_utf8(output.stdout.clone()).unwrap());

        let command = format!("{} {}", container_engine_command, shell_words::join(args));
        let status_code = output.status.code();
        match status_code {
            None => {
                let err = DockerError::CommandKilled { cmd: command };
                return Err(err);
            }
            Some(0) => (),
            Some(_n) => {
                let err = DockerError::CommandFailed {
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
}
