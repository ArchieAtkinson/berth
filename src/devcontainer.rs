use crate::{config::Environment, UnexpectedExt};
use log::info;
use miette::{Diagnostic, Result};
use semver::{Version, VersionReq};
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Output, Stdio},
};

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum DevContError {
    #[error("devcontainer cli not found")]
    #[diagnostic(code(cli::daemon), help("Is it installed?"))]
    CliNotFound(),

    #[error("devcontainer cli version must mmet {0} version requirements")]
    #[diagnostic(
        code(cli::daemon),
        help("Check the version with devcontainer --version")
    )]
    BadVersion(&'static str),

    #[error("The following command failed to run:\n{0}")]
    #[diagnostic(code(cli::container::command::failed))]
    CommandFailed(String),
}

struct CommandOut {
    status: i32,
    stdout: String,
    stderr: String,
}

pub struct DevContainer {
    env: Environment,
    _config_dir: PathBuf,
}

impl DevContainer {
    const CLI_PATH: &str = "devcontainer";
    const VERSION_REQ: &str = "^0.80.0";

    pub fn new<T: Into<PathBuf>>(env: Environment, config_path: T) -> Result<Self> {
        let mut config_dir = config_path.into();
        config_dir.pop();

        let output = Self::send_command(&["--version"])?;
        if !output.status.success() {
            return Err(DevContError::CliNotFound())?;
        }

        let mut stdout = String::from_utf8(output.stdout).unwrap();
        stdout.pop();
        let version_req = VersionReq::parse(Self::VERSION_REQ).unwrap();
        let version = Version::parse(&stdout).unwrap();

        if !version_req.matches(&version) {
            return Err(DevContError::BadVersion(Self::VERSION_REQ))?;
        }

        Ok(Self {
            env,
            _config_dir: config_dir,
        })
    }

    fn send_command(args: &[&str]) -> Result<Output> {
        let command = format!("{} {}", Self::CLI_PATH, shell_words::join(args));
        info!("{command}");

        let output = Command::new(Self::CLI_PATH)
            .args(args.iter().to_owned())
            .current_dir(std::env::current_dir().unwrap())
            .output()
            .expect("Command Failed");

        Ok(output)
    }

    fn send_command_with_stream(args: &[&str]) -> Result<CommandOut> {
        info!("{} {}", "devcontainer", args.join(" "));
        let mut cmd = Command::new("devcontainer")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let mut out = CommandOut {
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
        };

        {
            let stdout = cmd.stdout.as_mut().unexpected()?;
            let stdout_reader = BufReader::new(stdout);
            let stdout_lines = stdout_reader.lines();

            let stderr = cmd.stderr.as_mut().unexpected()?;
            let stderr_reader = BufReader::new(stderr);
            let stderr_lines = stderr_reader.lines();

            for (stdout, stderr) in stdout_lines.zip(stderr_lines) {
                let stdout = stdout.unwrap();
                println!("{}", &stdout);
                out.stdout.push_str(&stdout);

                let stderr = stderr.unwrap();
                println!("{}", &stderr);
                out.stderr.push_str(&stderr);
            }
        }

        cmd.wait().unwrap();

        Ok(out)
    }

    pub fn build(&self) -> Result<()> {
        let packages = self.env.nix_pkgs.join(",");

        let data = serde_json::json!({
            "ghcr.io/devcontainers/features/nix:1": {
                "useAttributePath": "true",
                "packages": packages
            }
        });

        let features = data.to_string().replace("\n", "");
        let mut args = vec![
            "up",
            "--workspace-folder",
            ".",
            "--additional-features",
            &features,
        ];

        if let Some(repo) = &self.env.dotfiles_repo {
            args.push("--dotfiles-repository");
            args.push(repo);

            if let Some(target_dir) = &self.env.dotfiles_target_dir {
                args.push("--dotfiles-target-path");
                args.push(target_dir);
            }

            if let Some(init_cmd) = &self.env.dotfiles_init_cmd {
                args.push("--dotfiles-install-command");
                args.push(init_cmd)
            }
        }

        let output = Self::send_command_with_stream(&args)?;
        info!("{}", output.stdout);
        info!("{}", output.stderr);
        info!("{}", output.status);

        if output.status != 0 {
            return Err(DevContError::CommandFailed(output.stderr))?;
        }

        Ok(())
    }

    pub fn enter() -> Result<i32> {
        let code = Command::new(Self::CLI_PATH)
            .args(["exec", "--workspace-folder", ".", "bash"])
            .current_dir(std::env::current_dir().unexpected()?)
            .status()
            .unexpected()?
            .code()
            .expect("Command Failed");
        info!("{}", code);
        Ok(code)
    }
}
