use berth::{
    cli::{AppConfig, Commands},
    configuration::{Configuration, Environment},
};
use miette::{GraphicalReportHandler, GraphicalTheme, Result};
use std::path::PathBuf;
use std::{io::Write, path::Path};
use tempfile::NamedTempFile;

pub struct ConfigTest {
    file_path: PathBuf,
    _file: Option<NamedTempFile>,
}

impl ConfigTest {
    pub fn new(config_content: &str) -> Self {
        let config_file = NamedTempFile::new().expect("Failed to create file for config");
        write!(&config_file, "{}", config_content).expect("Failed to write config file");

        ConfigTest {
            file_path: config_file.path().to_path_buf(),
            _file: Some(config_file),
        }
    }

    pub fn from_file(config_path: &Path) -> Self {
        ConfigTest {
            file_path: config_path.to_path_buf(),
            _file: None,
        }
    }

    pub fn get_env(&self, environment: &str) -> Result<Environment> {
        let app_config = AppConfig {
            config_path: self.file_path.clone(),
            command: Commands::Up {
                environment: environment.to_string(),
            },
            cleanup: true,
        };

        Configuration::new(&app_config)?.find_environment_from_configuration()
    }

    pub fn file_path(&self) -> &str {
        self.file_path.to_str().unwrap()
    }
}

pub trait ReportExt {
    fn render(&self) -> String;
}

impl ReportExt for miette::Report {
    fn render(&self) -> String {
        let mut out = String::new();
        GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            .render_report(&mut out, self.as_ref())
            .unwrap();
        out
    }
}
