use std::process::exit;

use berth::errors::AppError;
use berth::presets::Env;
use berth::{cli::AppConfig, docker::Docker, presets::Preset};

use log::info;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

fn init_logger() -> Result<(), Box<dyn std::error::Error>> {
    let file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d(%H:%M:%S)} - {l} - {m}\n")))
        .build("/tmp/berth.log")?;

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

fn find_environment_in_config(preset: &mut Preset, name: &str) -> Result<Env, AppError> {
    match preset.envs.remove(name) {
        Some(e) => Ok(e),
        None => Err(AppError::ProvidedEnvNameNotInConfig {
            name: name.to_string(),
        }),
    }
}

fn run() -> Result<(), AppError> {
    let args = std::env::args_os();
    let app_config = AppConfig::new(args)?;

    eprintln!("Using config file at {:?}", app_config.config_path);

    let config_content =
        std::fs::read_to_string(app_config.config_path).expect("Failed to read config file");
    let mut preset = Preset::new(&config_content)?;

    let env = find_environment_in_config(&mut preset, &app_config.env_name)?;

    let docker = Docker::new(env, app_config.no_tty);
    if !docker.does_environment_exist()? {
        docker.create_new_environment()?;
    } else {
        docker.start_container()?;
    }

    docker.enter_environment()?;

    if app_config.cleanup {
        docker.delete_container_if_exists()?;
    }

    Ok(())
}

fn main() {
    init_logger().expect("Failed to setup logger");

    info!("Start up");

    match run() {
        Ok(()) => (),
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    }

    info!("Done!");

    exit(0)
}
