use miette::{Diagnostic, LabeledSpan, NamedSource, Result, SourceSpan};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self},
};
use thiserror::Error;

use crate::{
    cli::{Action, Settings},
    util::UnexpectedExt,
};

#[derive(Debug, Error, PartialEq, Diagnostic)]
pub enum ConfigError {
    #[error("Malformed TOML")]
    #[diagnostic(code(configuration::parsing))]
    TomlParse {
        msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{msg}")]
        span: SourceSpan,
    },

    #[error("Malformed Environment")]
    #[diagnostic(code(configuration::environment::validation))]
    EnvironmentValidation {
        msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{msg}")]
        span: SourceSpan,
    },

    #[error("Environment Not Present")]
    #[diagnostic(code(configuration::environment::search))]
    EnvironmentSearch {
        msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{msg}")]
        span: SourceSpan,
    },

    #[error("Nonexistent Dockerfile")]
    #[diagnostic(code(configuration::environment::dockerfile))]
    InvalidDockerfilePath {
        msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{msg}")]
        span: SourceSpan,
    },

    #[error("Unknown Preset")]
    #[diagnostic(code(configuration::preset::unknown))]
    UnknownPreset {
        msg: String,
        #[source_code]
        input: NamedSource<String>,
        #[label("{msg}")]
        span: SourceSpan,
    },

    #[error("Duplicate Fields From Presets")]
    #[diagnostic(code(configuration::preset::duplication))]
    DuplicateFieldsFromPresets {
        #[source_code]
        input: NamedSource<String>,
        #[label(collection)]
        spans: Vec<LabeledSpan>,
    },

    #[error("Couldn't read provided dockerfile, '{0}', for hashing")]
    FailedToInteractWithDockerfile(String),
}

macro_rules! labeled_error {
    ($self:expr, $type: ident, $span:expr, $msg:expr) => {
        ConfigError::$type {
            input: NamedSource::new(
                $self.app.config_path.to_str().unwrap(),
                $self.content.to_string(),
            ),
            span: $span.into(),
            msg: $msg.to_string(),
        }
    };
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomlEnvironment {
    #[serde(default)]
    nix_pkgs: Vec<String>,
    #[serde(default)]
    dotfiles_repo: Option<String>,
    #[serde(default)]
    dotfiles_target_dir: Option<String>,
    #[serde(default)]
    dotfiles_init_cmd: Option<String>,
}

type TomlEnvs = HashMap<String, TomlEnvironment>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomlConfiguration {
    #[serde(rename = "environment")]
    pub environments: TomlEnvs,
}

#[derive(Hash, Debug, Clone)]
pub struct Environment {
    pub name: String,
    pub nix_pkgs: Vec<String>,
    pub dotfiles_repo: Option<String>,
    pub dotfiles_target_dir: Option<String>,
    pub dotfiles_init_cmd: Option<String>,
}

pub struct Config {
    content: String,
    app: Settings,
    doc: Option<toml_edit::ImDocument<String>>,
}

impl Config {
    pub fn new(app: &Settings) -> Result<Self> {
        let content = fs::read_to_string(&app.config_path).unexpected()?;
        Ok(Self {
            content,
            app: app.clone(),
            doc: None,
        })
    }

    pub fn find_environment_from_configuration(mut self) -> Result<Environment> {
        let config = self.parse_toml()?;
        self.create_environment(config.environments)
    }

    fn parse_toml(&mut self) -> Result<TomlConfiguration> {
        match toml_edit::de::from_str::<TomlConfiguration>(&self.content) {
            Ok(config) => {
                self.doc = Some(self.content.parse().unexpected()?);
                Ok(config)
            }
            Err(error) => {
                let span = error.span().unwrap();
                let label_message = match error.message() {
                    s if s.contains("missing field") => error.message(),
                    s if s.contains("unknown field") => "Unknown field",
                    s if s.contains("invalid type") => error.message(),
                    s if s.contains("duplicate key") => error.message(),
                    _ => &format!("Unexpected TOML Error {:?}", error.message()),
                };

                Err(labeled_error!(self, TomlParse, span, label_message).into())
            }
        }
    }

    fn create_environment(&self, mut envs: TomlEnvs) -> Result<Environment> {
        let Action::Init { environment: name } = self.app.action.clone() else {
            return Err(miette::miette!("Oops"))?;
        };

        let env = match envs.remove(&name) {
            Some(env) => env,
            None => {
                return Err(labeled_error!(
                    self,
                    EnvironmentSearch,
                    (0, self.content.len()),
                    format!("Failed to find provided environment '{}' in config", &name)
                )
                .into())
            }
        };

        let env = Environment {
            name: name.clone(),
            nix_pkgs: env.nix_pkgs,
            dotfiles_repo: env.dotfiles_repo,
            dotfiles_target_dir: env.dotfiles_target_dir,
            dotfiles_init_cmd: env.dotfiles_init_cmd,
        };

        Ok(env)
    }
}

impl Environment {
    pub fn view(&self) -> Result<String> {
        use toml_edit::{DocumentMut, Item};

        let mut doc = DocumentMut::new();
        let table = toml_edit::Table::new();

        let env_table = doc
            .as_table_mut()
            .entry("environment")
            .or_insert(Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .unexpected()?;

        env_table.set_dotted(true);
        env_table[&self.name] = Item::Table(table);

        Ok(doc.to_string())
    }
}
