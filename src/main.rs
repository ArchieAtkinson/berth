use berth::cli::{Action, AppConfig};
use berth::configuration2::Configuration;
use berth::devcontainer::DevContainer;
use log::info;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use miette::{miette, Result};

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

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().expect("Failed to setup logger");

    info!("Start up");

    let args = std::env::args_os();
    let app_config = AppConfig::new(args)?;

    if app_config.action == Action::Up {
        let status = DevContainer::enter()?;
        if status == 0 {
            eprintln!("Goodbye!");
            return Ok(());
        } else {
            return Err(miette!("Exited with:{}", status));
        }
    }

    eprintln!("Using config file at {:?}", app_config.config_path);

    let environment = Configuration::new(&app_config)?.find_environment_from_configuration()?;

    let dev = DevContainer::new(environment.clone(), app_config.config_path)?;

    match app_config.action {
        Action::Init { environment: _ } => {
            dev.build().unwrap();
        }
        _ => panic!("Haven't gotten to that yet"),
    };

    info!("Done!");

    Ok(())
}
