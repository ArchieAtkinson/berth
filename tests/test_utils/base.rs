use color_eyre::{eyre::eyre, Result};
use eyre::Context;
use rand::{thread_rng, Rng};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;

use super::BINARY;

pub(crate) struct TestBase {
    pub(crate) config_path: PathBuf,
    pub(crate) tmp_config_file: Option<NamedTempFile>,
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
    pub(crate) working_dir: Option<PathBuf>,
    pub(crate) envs: Vec<(String, String)>,
    pub(crate) command_string: String,
    pub(crate) replacements: HashMap<String, String>,
}

impl TestBase {
    pub fn new() -> Self {
        let name = Self::generate_random_environment_name();
        let replacements = HashMap::from([("[name]".to_string(), name.clone())]);

        Self {
            config_path: PathBuf::new(),
            tmp_config_file: None,
            name,
            args: Vec::new(),
            working_dir: None,
            envs: Vec::new(),
            command_string: String::new(),
            replacements,
        }
    }

    #[track_caller]
    pub fn config(&mut self, content: &str) -> Result<&mut Self> {
        let tmp_file =
            NamedTempFile::new().wrap_err("Failed to create temporary file for config")?;
        let path = tmp_file.path().to_path_buf();
        self.replacements.insert(
            "[config_path]".to_string(),
            tmp_file.path().display().to_string(),
        );
        self.tmp_config_file = Some(tmp_file);
        self.config_with_path(content, &path)
    }

    #[track_caller]
    pub fn config_with_path(&mut self, content: &str, path: &Path) -> Result<&mut Self> {
        fs::write(
            path,
            format!("[environment.\"{}\"]\n{}", &self.name, content),
        )
        .wrap_err("Failed to write config to temporary file")?;
        self.config_path = path.to_path_buf();
        Ok(self)
    }

    #[track_caller]
    pub fn args(&mut self, args: Vec<&str>) -> Result<&mut Self> {
        self.args = args
            .into_iter()
            .map(|arg| {
                match self.replacements.get(arg) {
                    Some(value) => value,
                    None => arg,
                }
                .to_string()
            })
            .collect();
        Ok(self)
    }

    #[track_caller]
    pub fn envs(&mut self, envs: Vec<(&str, &str)>) -> Result<&mut Self> {
        self.envs.extend(
            envs.iter()
                .map(|s| (s.0.to_string(), s.1.to_string()))
                .collect::<Vec<(String, String)>>(),
        );
        Ok(self)
    }

    #[track_caller]
    pub fn working_dir(&mut self, working_dir: &str) -> Result<&mut Self> {
        let full_path = fs::canonicalize(working_dir)
            .wrap_err("Failed to create canonical path for working directory")?;
        if !full_path.exists() {
            return Err(eyre!("Provided working directory does not exist"));
        }
        if !full_path.is_dir() {
            return Err(eyre!("Provided working directory is not a directory"));
        }

        self.working_dir = Some(full_path);
        Ok(self)
    }

    #[track_caller]
    pub fn create_command(&mut self) -> Result<Command> {
        let bin_path = assert_cmd::cargo::cargo_bin(BINARY);
        let mut command = Command::new(&bin_path);

        if let Some(dir) = &self.working_dir {
            command.current_dir(dir);
        }

        if let Ok(docker_host) = std::env::var("DOCKER_HOST") {
            self.envs.push(("DOCKER_HOST".into(), docker_host));
        }

        command.env_clear();
        command.args(self.args.clone());
        command.envs(self.envs.clone());

        let command_vec: Vec<String> = std::iter::once(bin_path.display().to_string())
            .chain(self.args.clone())
            .collect();
        self.command_string = shell_words::join(command_vec);
        Ok(command)
    }
}

impl TestBase {
    #[must_use]
    pub fn config_path(&self) -> &str {
        self.config_path.to_str().unwrap()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    fn generate_random_environment_name() -> String {
        const LENGTH: usize = 32;
        let mut rng = thread_rng();

        // Environment containers already have a prefix
        // this extra one is to show its used in testing
        let first_chars: &str = "test-";

        // Characters for the rest of the positions [a-zA-Z0-9_.-]
        let other_chars: Vec<char> = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .map(char::from)
            .collect();

        let rest: String = (0..LENGTH)
            .map(|_| other_chars[rng.gen_range(0..other_chars.len())])
            .collect();

        format!("{}{}", first_chars, rest)
    }

    fn drop(&mut self) {
        if !self.name().is_empty() {
            let name_arg = format!("name=berth-{}", self.name());
            let containers = Command::new("docker")
                .args(["ps", "-a", "--filter", &name_arg, "--format", "{{.Names}}"])
                .output()
                .unwrap();
            let container = String::from_utf8(containers.stdout)
                .unwrap()
                .trim()
                .to_string();
            if !container.is_empty() {
                println!("Deleting container: {}", container);
                Command::new("docker")
                    .args(["rm", "-f", &container])
                    .output()
                    .unwrap();
            }

            let image_name = format!("berth-{}*", &self.name().to_lowercase());
            let images = Command::new("docker")
                .args(["images", "-q", &image_name])
                .output()
                .unwrap();
            let image = String::from_utf8(images.stdout).unwrap().trim().to_string();
            if !image.is_empty() {
                println!("Deleting image: {}", &image);
                Command::new("docker")
                    .args(["image", "rm", "-f", &image])
                    .output()
                    .unwrap();
            }
        }
    }
}

impl Drop for TestBase {
    fn drop(&mut self) {
        self.drop();
    }
}

impl Default for TestBase {
    fn default() -> Self {
        Self::new()
    }
}
