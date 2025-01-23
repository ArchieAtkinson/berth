use std::{
    env,
    path::{Path, PathBuf},
};

use clap::Parser;
use berth::{arguments::Arguments, docker, presets::Preset};

use log::info;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

fn init_logger() -> Result<(), Box<dyn std::error::Error>> {
    let file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%H:%M:%S%.3f)} - {l} - {m}\n",
        )))
        .build("log/output.log")?;

    let config = Config::builder()
        .appender(Appender::builder().build("file", Box::new(file)))
        .build(
            Root::builder()
                .appender("file")
                .build(log::LevelFilter::Info),
        )?;

    log4rs::init_config(config)?;

    Ok(())
}

fn get_config_path(config_path: &Path) -> Result<PathBuf, String> {
    if config_path.exists() {
        return Ok(config_path.to_path_buf());
    }

    if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
        let xdg_path = Path::new(&xdg_config).join(".config").join("ch.config");
        if xdg_path.exists() {
            return Ok(xdg_path);
        }
    }

    if let Ok(home) = env::var("HOME") {
        let home_path = Path::new(&home).join(".config").join("ch.config");
        if home_path.exists() {
            return Ok(home_path);
        }
    }

    Err("Could not find config file in any of the expected locations".to_string())
}

fn main() {
    init_logger().unwrap();

    info!("Start up");

    let args = Arguments::parse();
    let config_path_arg = args.config_file.unwrap();

    let config_path = get_config_path(&config_path_arg).unwrap();
    info!("Using config file at {:?}", config_path);

    let config_content = std::fs::read_to_string(config_path).unwrap();
    let preset = Preset::new(&config_content).unwrap();

    if preset.env.is_empty() {
        eprintln!("No environments configured!");
        return;
    }

    docker::enter(&args.env_name, preset.env).unwrap();
}
