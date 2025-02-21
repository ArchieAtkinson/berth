use envmnt::{ExpandOptions, ExpansionType};
use miette::{Diagnostic, NamedSource, Result, SourceSpan};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::cli::{AppConfig, Commands};

#[derive(Debug, Error, PartialEq, Diagnostic)]
pub enum ConfigError {
    #[error("Malformed TOML")]
    #[diagnostic(code(configuration::parsing))]
    TomlParse {
        toml_msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{toml_msg}")]
        span: SourceSpan,
    },

    #[error("Environment '{0}' has specified dockerfile or image")]
    DockerfileOrImage(String),

    #[error("Environment '{0}' has not specified a dockerfile or image")]
    RequireDockerfileOrImage(String),

    #[error("Can't find dockerfile at: {0}")]
    BadDockerfilePath(String),

    #[error("Proved environment, '{0}' is not in loaded config")]
    ProvidedEnvNameNotInConfig(String),

    #[error("Couldn't read provided dockerfile, '{0}', for hashing")]
    FailedToInteractWithDockerfile(String),
}

#[derive(Debug, Deserialize, Hash)]
#[serde(deny_unknown_fields)]
pub struct TomlEnvironment {
    entry_cmd: String,

    #[serde(default)]
    #[serde(rename = "image")]
    provided_image: String,

    #[serde(default)]
    dockerfile: String,

    #[serde(default)]
    entry_options: Vec<String>,

    #[serde(default)]
    exec_cmds: Vec<String>,

    #[serde(default)]
    exec_options: Vec<String>,

    #[serde(default)]
    create_options: Vec<String>,
}

type TomlEnvs = HashMap<String, TomlEnvironment>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomlConfiguration {
    #[serde(rename = "environment")]
    pub environments: TomlEnvs,
}

pub struct Configuration {}

#[derive(Hash, Debug)]
pub struct Environment {
    pub name: String,
    pub image: String,
    pub dockerfile: Option<PathBuf>,
    pub entry_cmd: String,
    pub entry_options: Vec<String>,
    pub exec_cmds: Vec<String>,
    pub exec_options: Vec<String>,
    pub create_options: Vec<String>,
}

impl Configuration {
    pub fn find_environment_from_configuration(app: &AppConfig) -> Result<Environment> {
        let file_content =
            fs::read_to_string(&app.config_path).expect("Failed to read config file");

        match toml::from_str::<TomlConfiguration>(&file_content) {
            Ok(mut config) => {
                Self::check_toml_environments_are_valid(&config.environments)?;
                Ok(Self::create_environment(&mut config.environments, app)?)
            }
            Err(e) => Err(Self::custom_parse_error(
                &file_content,
                &app.config_path,
                &e,
            )),
        }
    }

    fn custom_parse_error(content: &str, file: &Path, error: &toml::de::Error) -> miette::Report {
        let span = error.span().unwrap();

        let label_message = match error.message() {
            s if s.contains("missing field") => error.message(),
            s if s.contains("unknown field") => "Unknown field",
            s if s.contains("invalid type") => error.message(),
            s if s.contains("duplicate key") => error.message(),
            _ => &format!("Unexpected TOML Error {:?}", error.message()),
        };

        ConfigError::TomlParse {
            input: NamedSource::new(file.to_str().unwrap(), content.to_string()),
            span: span.into(),
            toml_msg: label_message.to_string(),
        }
        .into()
    }

    fn check_toml_environments_are_valid(envs: &TomlEnvs) -> Result<()> {
        for (name, env) in envs {
            match (env.provided_image.is_empty(), env.dockerfile.is_empty()) {
                (true, true) => {
                    return Err(ConfigError::RequireDockerfileOrImage(name.clone()).into())
                }
                (false, false) => return Err(ConfigError::DockerfileOrImage(name.clone()).into()),
                _ => (),
            }
        }

        Ok(())
    }

    fn create_environment(config: &mut TomlEnvs, app: &AppConfig) -> Result<Environment> {
        let name = match app.command.clone() {
            Commands::Up { environment: e } => e,
            Commands::Build { environment: e } => e,
        };

        let mut env = config
            .remove(&name)
            .ok_or(ConfigError::ProvidedEnvNameNotInConfig(name.to_string()))?;

        Self::expand_environment_variables(&mut env);

        let (image, dockerfile) = if env.provided_image.is_empty() {
            let dockerfile_path = Self::parse_dockerfile(&env.dockerfile, &app.config_path)?;
            let image_name = Self::generate_image_name(&name, &dockerfile_path)?;
            (image_name, Some(dockerfile_path))
        } else {
            (env.provided_image, None)
        };

        let mut env = Environment {
            name: name.to_string(),
            image,
            dockerfile,
            entry_cmd: env.entry_cmd,
            entry_options: env.entry_options,
            exec_cmds: env.exec_cmds,
            exec_options: env.exec_options,
            create_options: env.create_options,
        };

        let mut hasher = DefaultHasher::new();
        env.hash(&mut hasher);
        env.name = format!("{}-{}-{:016x}", "berth", &name, hasher.finish());

        Ok(env)
    }

    fn expand_environment_variables(env: &mut TomlEnvironment) {
        Self::expand_env_vars(&mut env.entry_options);
        Self::expand_env_vars(&mut env.exec_options);
        Self::expand_env_vars(&mut env.create_options);
    }

    fn parse_dockerfile(dockerfile: &str, config_path: &Path) -> Result<PathBuf> {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        let dockerfile = envmnt::expand(dockerfile, Some(options));

        let path = Path::new(&dockerfile);

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let config_dir =
                config_path
                    .parent()
                    .ok_or(ConfigError::FailedToInteractWithDockerfile(
                        path.display().to_string(),
                    ))?;
            config_dir.join(path)
        };

        if !resolved.exists() || !resolved.is_file() {
            return Err(ConfigError::BadDockerfilePath(resolved.display().to_string()).into());
        }

        Ok(resolved)
    }

    fn generate_image_name(name: &str, path: &Path) -> Result<String> {
        let create_error = |path: &Path| -> miette::Report {
            ConfigError::FailedToInteractWithDockerfile(path.display().to_string()).into()
        };

        let path = fs::canonicalize(path).map_err(|_| create_error(path))?;
        let mut file = File::open(&path).map_err(|_| create_error(&path))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 1024];

        loop {
            let bytes_read = file.read(&mut buffer).map_err(|_| create_error(&path))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!(
            "{}-{}-{:016x}",
            "berth",
            name.to_lowercase(),
            hasher.finalize()
        ))
    }

    fn expand_env_vars(vec: &mut [String]) {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        vec.iter_mut()
            .for_each(|s| *s = envmnt::expand(s, Some(options)));
    }
}
