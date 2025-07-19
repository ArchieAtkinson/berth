use envmnt::{ExpandOptions, ExpansionType};
use miette::{Diagnostic, LabeledSpan, NamedSource, Result, SourceSpan};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::Read,
    ops::Range,
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::{cli::AppConfig, util::UnexpectedExt};

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
    entry_cmd: String,

    #[serde(default)]
    #[serde(rename = "image")]
    provided_image: String,

    #[serde(default)]
    dockerfile: String,

    #[serde(default)]
    entry_options: Vec<String>,

    #[serde(default)]
    cp_cmds: Vec<String>,

    #[serde(default)]
    exec_cmds: Vec<String>,

    #[serde(default)]
    exec_options: Vec<String>,

    #[serde(default)]
    create_options: Vec<String>,

    #[serde(default)]
    presets: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TomlPreset {
    #[serde(default)]
    entry_cmd: String,

    #[serde(default)]
    #[serde(rename = "image")]
    provided_image: String,

    #[serde(default)]
    dockerfile: String,

    #[serde(default)]
    entry_options: Vec<String>,

    #[serde(default)]
    cp_cmds: Vec<String>,

    #[serde(default)]
    exec_cmds: Vec<String>,

    #[serde(default)]
    exec_options: Vec<String>,

    #[serde(default)]
    create_options: Vec<String>,
}

type TomlEnvs = HashMap<String, TomlEnvironment>;
type TomlPresets = HashMap<String, TomlPreset>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomlConfiguration {
    #[serde(rename = "environment")]
    pub environments: TomlEnvs,
    #[serde(rename = "preset", default)]
    pub presets: TomlPresets,
}

#[derive(Hash, Debug, Clone)]
pub struct Environment {
    pub name: String,
    pub original_name: String,
    pub image: String,
    pub dockerfile: Option<PathBuf>,
    pub entry_cmd: String,
    pub entry_options: Vec<String>,
    pub exec_cmds: Vec<String>,
    pub exec_options: Vec<String>,
    pub create_options: Vec<String>,
    pub cp_cmds: Vec<String>,
}

pub struct Configuration {
    content: String,
    app: AppConfig,
    doc: Option<toml_edit::ImDocument<String>>,
}

impl Configuration {
    pub fn new(app: &AppConfig) -> Result<Self> {
        let content = fs::read_to_string(&app.config_path).unexpected()?;
        Ok(Self {
            content,
            app: app.clone(),
            doc: None,
        })
    }

    pub fn find_environment_from_configuration(mut self) -> Result<Environment> {
        let config = self.parse_toml()?;
        let config = self.check_presets_exist(config)?;
        let config = self.valid_unique_fields(config)?;
        let envs = self.merge_presets(config)?;
        let envs = self.validate_environments(envs)?;
        self.create_environment(envs)
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

    fn check_presets_exist(&self, config: TomlConfiguration) -> Result<TomlConfiguration> {
        for (env_name, env) in &config.environments {
            for preset_name in &env.presets {
                if !config.presets.contains_key(preset_name) {
                    let span = self
                        .doc
                        .as_ref()
                        .unexpected()?
                        .get("environment")
                        .and_then(|env| env.as_table())
                        .and_then(|table| table.get(env_name))
                        .and_then(|item| item.get("presets"))
                        .and_then(|item| item.as_array())
                        .and_then(|array| {
                            array
                                .iter()
                                .find(|v| v.as_str() == Some(preset_name))
                                .and_then(|value| value.span())
                        })
                        .unexpected()?;
                    return Err(labeled_error!(
                        self,
                        UnknownPreset,
                        span,
                        "Failed to find provided preset"
                    )
                    .into());
                }
            }
        }
        Ok(config)
    }

    fn valid_unique_fields(&self, config: TomlConfiguration) -> Result<TomlConfiguration> {
        let find_fields_span = |table: &str, name: &str, field: &str| -> Result<SourceSpan> {
            let span = self
                .doc
                .as_ref()
                .unexpected()?
                .get(table)
                .and_then(|envs_item| envs_item.as_table())
                .and_then(|envs_table| envs_table.get(name))
                .and_then(|env_item| env_item.as_table())
                .and_then(|env_table| env_table.get_key_value(field))
                .map(|(key, value)| {
                    let key_span = key.span().unwrap();
                    let value_span = value.span().unwrap();
                    key_span.start..value_span.end
                })
                .unexpected()?;
            Ok(span.into())
        };

        let check_unique = |field: &str, env: &TomlEnvironment, env_name: &str| -> Result<()> {
            let mut spans: Vec<LabeledSpan> = Vec::new();

            let is_env_field_preset = match field {
                "entry_cmd" => !env.entry_cmd.is_empty(),
                "image" => !env.provided_image.is_empty(),
                "dockerfile" => !env.dockerfile.is_empty(),
                _ => unreachable!("Unknown field {field}"),
            };

            if is_env_field_preset {
                let span = find_fields_span("environment", env_name, field)?;
                let text = format!("instance {}", spans.len() + 1);
                let labeled_span = LabeledSpan::new_with_span(Some(text), span);
                spans.push(labeled_span);
            }

            // Using sorted preset names isn't required but helps testings as the
            // the HashMap doesn't guarantee the order
            let mut sorted_preset_names: Vec<_> = config.presets.keys().collect();
            sorted_preset_names.sort();

            for preset_name in sorted_preset_names {
                let is_preset_field_preset = match field {
                    "entry_cmd" => !config.presets[preset_name].entry_cmd.is_empty(),
                    "image" => !config.presets[preset_name].provided_image.is_empty(),
                    "dockerfile" => !config.presets[preset_name].dockerfile.is_empty(),
                    _ => unreachable!("Unknown field {field}"),
                };

                if is_preset_field_preset {
                    let span = find_fields_span("preset", preset_name, field)?;
                    let text = format!("instance {}", spans.len() + 1);
                    let labeled_span = LabeledSpan::new_with_span(Some(text), span);
                    spans.push(labeled_span);
                }
            }

            // If we more than 1 span, then we have duplicate fields
            // if zero, then non are present which is fine for some fields
            // and is handled later.
            if spans.len() > 1 {
                let span = self
                    .doc
                    .as_ref()
                    .unexpected()?
                    .get("environment")
                    .and_then(|env| env.as_table())
                    .and_then(|table| table.get(env_name))
                    .and_then(|item| item.get("presets"))
                    .and_then(|item| item.span())
                    .unexpected()?;

                let labeled_span = LabeledSpan::new_with_span(
                    Some(format!("Preset(s) causing duplicate '{field}' field")),
                    span,
                );
                spans.push(labeled_span);

                return Err(ConfigError::DuplicateFieldsFromPresets {
                    input: NamedSource::new(
                        self.app.config_path.to_str().unwrap(),
                        self.content.to_string(),
                    ),
                    spans,
                }
                .into());
            }
            Ok(())
        };

        for (env_name, env) in &config.environments {
            check_unique("entry_cmd", env, env_name)?;
            check_unique("image", env, env_name)?;
            check_unique("dockerfile", env, env_name)?;
        }
        Ok(config)
    }

    fn merge_presets(&self, mut config: TomlConfiguration) -> Result<TomlEnvs> {
        for (_, env) in config.environments.iter_mut() {
            for preset_name in env.presets.iter_mut() {
                let preset = config.presets.get(preset_name).unexpected()?;

                if !preset.entry_cmd.is_empty() {
                    env.entry_cmd = preset.entry_cmd.clone();
                }

                if !preset.provided_image.is_empty() {
                    env.provided_image = preset.provided_image.clone();
                }

                if !preset.dockerfile.is_empty() {
                    env.dockerfile = preset.dockerfile.clone();
                }

                env.entry_options.extend_from_slice(&preset.entry_options);
                env.exec_cmds.extend_from_slice(&preset.exec_cmds);
                env.exec_options.extend_from_slice(&preset.exec_options);
                env.create_options.extend_from_slice(&preset.create_options);
                env.cp_cmds.extend_from_slice(&preset.cp_cmds);
            }
        }

        Ok(config.environments)
    }

    fn validate_environments(&self, envs: TomlEnvs) -> Result<TomlEnvs> {
        let get_span = move |env_name: &str| -> Result<Range<usize>> {
            self.doc
                .as_ref()
                .unexpected()?
                .get("environment")
                .and_then(|env| env.as_table())
                .and_then(|table| table.get(env_name))
                .and_then(|item| item.span())
                .unexpected()
        };

        for (name, env) in &envs {
            if env.entry_cmd.is_empty() {
                return Err(labeled_error!(
                    self,
                    EnvironmentValidation,
                    get_span(name)?,
                    "An environment requires a 'entry_cmd' field"
                )
                .into());
            }

            match (env.provided_image.is_empty(), env.dockerfile.is_empty()) {
                (true, true) => {
                    return Err(labeled_error!(
                        self,
                        EnvironmentValidation,
                        get_span(name)?,
                        "An environment requires an 'image' or 'dockerfile' field"
                    )
                    .into())
                }
                (false, false) => {
                    return Err(labeled_error!(
                        self,
                        EnvironmentValidation,
                        get_span(name)?,
                        "An environment can only have an 'image' or 'dockerfile' field"
                    )
                    .into())
                }

                _ => (),
            }
        }

        Ok(envs)
    }

    fn create_environment(&self, mut envs: TomlEnvs) -> Result<Environment> {
        let name = self.app.environment.clone();

        let mut env = match envs.remove(&name) {
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

        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        [
            &mut env.entry_options,
            &mut env.exec_options,
            &mut env.create_options,
        ]
        .iter_mut()
        .for_each(|vec| {
            vec.iter_mut()
                .for_each(|s| *s = envmnt::expand(s, Some(options)))
        });

        let (image, dockerfile) = match env.provided_image.as_str() {
            "" => {
                let dockerfile_path = self.validate_dockerfile(&env.dockerfile, &name)?;
                let image_name = Self::generate_image_name(&name, &dockerfile_path)?;
                (image_name, Some(dockerfile_path))
            }
            _ => (env.provided_image, None),
        };

        let mut env = Environment {
            name: name.to_string(),
            original_name: name.to_string(),
            image,
            dockerfile,
            entry_cmd: env.entry_cmd,
            entry_options: env.entry_options,
            exec_cmds: env.exec_cmds,
            exec_options: env.exec_options,
            create_options: env.create_options,
            cp_cmds: env.cp_cmds,
        };

        let mut hasher = DefaultHasher::new();
        env.hash(&mut hasher);
        env.name = format!("{}-{}-{:016x}", "berth", &name, hasher.finish());

        Ok(env)
    }

    fn validate_dockerfile(&self, dockerfile: &str, env_name: &str) -> Result<PathBuf> {
        let mut options = ExpandOptions::new();
        options.expansion_type = Some(ExpansionType::Unix);

        let dockerfile = envmnt::expand(dockerfile, Some(options));

        let path = Path::new(&dockerfile);

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.app
                .config_path
                .parent()
                .ok_or_else(|| {
                    ConfigError::FailedToInteractWithDockerfile(path.display().to_string())
                })?
                .join(path)
        };

        if !resolved.exists() || !resolved.is_file() {
            let span = self
                .doc
                .as_ref()
                .unexpected()?
                .get("environment")
                .and_then(|env| env.as_table())
                .and_then(|envs| envs.get(env_name))
                .and_then(|env| env.get("dockerfile"))
                .and_then(|item| item.span())
                .unexpected()?;

            return Err(labeled_error!(
                self,
                InvalidDockerfilePath,
                span,
                "Could not find dockerfile"
            )
            .into());
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
}

impl Environment {
    pub fn view(&self) -> Result<String> {
        use toml_edit::{value, Array, DocumentMut, Item};

        let mut doc = DocumentMut::new();
        let mut table = toml_edit::Table::new();

        if !self.image.is_empty() && self.dockerfile.is_none() {
            table.insert("image", value(self.image.clone()));
        }

        if let Some(path) = &self.dockerfile {
            table.insert("dockerfile", value(path.display().to_string()));
        }

        table.insert("entry_cmd", value(self.entry_cmd.clone()));

        if !self.entry_options.is_empty() {
            table.insert(
                "entry_options",
                value(Array::from_iter(self.entry_options.iter())),
            );
        }

        if !self.exec_cmds.is_empty() {
            table.insert("exec_cmds", value(Array::from_iter(self.exec_cmds.iter())));
        }

        if !self.exec_options.is_empty() {
            table.insert(
                "exec_options",
                value(Array::from_iter(self.exec_options.iter())),
            );
        }

        if !self.create_options.is_empty() {
            table.insert(
                "create_options",
                value(Array::from_iter(self.create_options.iter())),
            );
        }

        let env_table = doc
            .as_table_mut()
            .entry("environment")
            .or_insert(Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .unexpected()?;

        env_table.set_dotted(true);
        env_table[&self.original_name] = Item::Table(table);

        Ok(doc.to_string())
    }
}
