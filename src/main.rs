use berth::cli::{Action, Settings};
use berth::config::Config;
use berth::devcontainer::DevContainer;
use berth::env::SysEnv;
use log::info;
use miette::{miette, Result};

fn init_logger() -> Result<(), Box<dyn std::error::Error>> {
    use log4rs::append::file::FileAppender;
    use log4rs::config::{Appender, Config, Root};
    use log4rs::encode::pattern::PatternEncoder;

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

    eprintln!("\nBerth Log File Resumed At: /tmp/berth.log\n");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().expect("Failed to setup logger");

    info!("Start up");

    let args = std::env::args_os();
    let env = SysEnv::new();

    let settings = Settings::new(args, &env)?;

    if settings.action == Action::Up {
        let status = DevContainer::enter()?;
        if status == 0 {
            eprintln!("Goodbye!");
            return Ok(());
        } else {
            return Err(miette!("Exited with:{}", status));
        }
    }

    eprintln!("Using config file at {:?}", settings.config_path);

    let config = Config::new(&settings)?;
    let environment = config.find_environment_from_configuration()?;

    let dev = DevContainer::new(environment.clone(), settings.config_path)?;

    match settings.action {
        Action::Init { environment: _ } => {
            dev.build()?;
        }
        _ => panic!("Haven't gotten to that yet"),
    };

    info!("Done!");

    Ok(())
}
