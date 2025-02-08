use std::process::exit;

use berth::cli;
use berth::errors::AppError;
use berth::configuration::Environment;
use berth::{cli::AppConfig, docker::ContainerEngine, configuration::Configuration};

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

fn find_environment_in_config(configuration: &mut Configuration, name: &str) -> Result<Environment, AppError> {
    match configuration.environments.remove(name) {
        Some(e) => Ok(e),
        None => Err(AppError::ProvidedEnvNameNotInConfig {
            name: name.to_string(),
        }),
    }
}

async fn build(docker: &ContainerEngine) -> Result<(), AppError> {
    docker.create_new_environment().await?;

    if docker.is_container_running().await? && !docker.is_anyone_connected().await? {
        docker.stop_container().await?;
    }

    Ok(())
}

async fn up(docker: &ContainerEngine) -> Result<(), AppError> {
    if !docker.does_environment_exist().await? {
        docker.create_new_environment().await?;
    } else {
        docker.start_container().await?;
    }
    docker.enter_environment().await?;

    Ok(())
}

async fn run() -> Result<(), AppError> {
    let args = std::env::args_os();
    let app_config = AppConfig::new(args)?;

    eprintln!("Using config file at {:?}", app_config.config_path);

    let config_content =
        std::fs::read_to_string(app_config.config_path).expect("Failed to read config file");
    let mut configuration = Configuration::new(&config_content)?;

    let name = match &app_config.command {
        cli::Commands::Up { environment } => environment,
        cli::Commands::Build { environment } => environment,
    };

    let env = find_environment_in_config(&mut configuration, &name)?;
    let docker = ContainerEngine::new(env)?;

    let result = {
        match &app_config.command {
            cli::Commands::Up { environment: _ } => up(&docker).await,
            cli::Commands::Build { environment: _ } => build(&docker).await,
        }
    };

    match result {
        Ok(_) => Ok::<(), AppError>(()),
        Err(e) => {
            docker.stop_container().await?;
            return Err(e);
        }
    }?;

    if app_config.cleanup {
        docker.delete_container_if_exists().await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    init_logger().expect("Failed to setup logger");

    info!("Start up");

    match run().await {
        Ok(()) => (),
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    }

    info!("Done!");

    exit(0)
}
