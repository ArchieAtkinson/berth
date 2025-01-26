use std::process::exit;

use berth::errors::AppError;
use berth::presets::Env;
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

fn find_enviroment_in_config(preset: &mut Preset, name: &str) -> Result<Env, AppError> {
    match preset.env.remove(name) {
        Some(e) => Ok(e),
        None => Err(AppError::ProvidedEnvNameNotInConfig {
            name: name.to_string(),
        }),
    }
}

fn run() -> Result<(), AppError> {
    let args = std::env::args_os();
    let env_vars = EnvVar::new(std::env::vars());
    let app_config = AppConfig::new(args, &env_vars)?;

    println!("Using config file at {:?}", app_config.config_path);

    let config_content = std::fs::read_to_string(app_config.config_path).unwrap();
    let mut preset = Preset::new(&config_content)?;

    let env = find_enviroment_in_config(&mut preset, &app_config.env_name)?;

    Ok(docker::enter(env)?)
}

fn main() {
    init_logger().unwrap();

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
