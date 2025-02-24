use berth::cli;
use berth::util::Spinner;
use berth::{cli::AppConfig, configuration::Configuration, docker::DockerHandler};
use log::info;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use miette::Result;

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

async fn build(docker: &DockerHandler) -> Result<()> {
    docker.create_new_environment().await?;
    docker.stop_container().await?;

    Ok(())
}

async fn up(docker: &DockerHandler) -> Result<()> {
    if !docker.does_environment_exist().await? {
        docker.create_new_environment().await?;
    } else {
        let spinner = Spinner::new("Starting Container");

        docker.start_container().await?;

        spinner.finish_and_clear();
    }
    docker.enter_environment().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().expect("Failed to setup logger");

    info!("Start up");

    let args = std::env::args_os();
    let app_config = AppConfig::new(args)?;

    eprintln!("Using config file at {:?}", app_config.config_path);

    let environment = Configuration::new(&app_config)?.find_environment_from_configuration()?;

    let docker = DockerHandler::new(environment)?;

    let result = {
        match &app_config.command {
            cli::Commands::Up { environment: _ } => up(&docker).await,
            cli::Commands::Build { environment: _ } => build(&docker).await,
        }
    };

    if let Err(command_error) = result {
        if let Err(stop_error) = docker.stop_container().await {
            return Err(command_error.wrap_err(stop_error));
        }
    }

    if app_config.cleanup {
        docker.delete_container_if_exists().await?;
    }

    info!("Done!");

    Ok(())
}
