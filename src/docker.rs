use crate::presets::Env;
use log::info;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("The following command failed:\n {cmd}\nWith:\n{stderr}")]
    CommandFailed { cmd: String, stderr: String },

    #[error("The following command failed:\n {cmd}\nDue to an unknown signal")]
    CommandKilled { cmd: String },
}

struct Docker {
    env: Env,
}

impl Docker {
    pub fn new(env: Env) -> Self {
        Docker { env }
    }
    pub fn setup_new_container(&self) -> Result<(), DockerError> {
        self.delete_container_if_exists()?;
        self.create_container()?;
        self.start_container()?;
        self.exec_setup_commands()
    }

    fn create_container(&self) -> Result<(), DockerError> {
        let mut create_args = vec!["create", "-it", "--name", &self.env.name];

        if let Some(user) = &self.env.user {
            create_args.extend_from_slice(&["-u", user]);
        }

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

    pub fn enter_container(&self) -> Result<(), DockerError> {
        let args = vec!["exec", "-it", &self.env.name, &self.env.init_cmd];
        Command::new("docker").args(&args).status().unwrap();

        info!("docker {:?}", args);

        Ok(())
    }

    fn delete_container_if_exists(&self) -> Result<(), DockerError> {
        let filter = format!("name={}", &self.env.name);
        let ls_args = vec!["container", "ls", "-a", "--quiet", "--filter", &filter];

        let ls_output = Command::new("docker").args(&ls_args).output().unwrap();

        if ls_output.stdout.is_empty() {
            return Ok(());
        }

        let rm_args = vec!["container", "rm", &self.env.name];
        Self::run_docker_command(rm_args)
    }

    fn run_docker_command(args: Vec<&str>) -> Result<(), DockerError> {
        let container_engine_command = "docker";
        let output = Command::new(container_engine_command)
            .args(&args)
            .output()
            .unwrap();
        let mut command = vec![container_engine_command];
        command.extend(args);
        let command = shell_words::join(command);

        info!("{:#?}", String::from_utf8(output.stdout.clone()));

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
        Ok(())
    }

    pub fn stop_container(&self) -> Result<(), DockerError> {
        Self::run_docker_command(vec!["stop", "-t", "0", &self.env.name])
    }
}

pub fn enter(name: &str, envs: Vec<Env>) -> Result<(), DockerError> {
    let env = envs.into_iter().find(|e| e.name == name).unwrap();

    let docker = Docker::new(env);

    docker.setup_new_container()?;

    docker.enter_container()?;

    docker.stop_container()
}
