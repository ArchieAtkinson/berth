use std::process::exit;

use berth::util::EnvVar;
use berth::{cli::AppConfig, docker, presets::Preset};

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

fn main() {
    init_logger().unwrap();

    info!("Start up");

    let args = std::env::args_os();
    let env_vars = EnvVar::new(std::env::vars());
    let app_config = match AppConfig::new(args, &env_vars) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };

    info!("Using config file at {:?}", app_config.config_path);

    let config_content = std::fs::read_to_string(app_config.config_path).unwrap();
    let preset = Preset::new(&config_content).unwrap();

    if preset.env.is_empty() {
        eprintln!("No environments configured!");
        return;
    }

    docker::enter(&app_config.env_name, preset.env).unwrap();

    info!("Done!");

    exit(0)
}
