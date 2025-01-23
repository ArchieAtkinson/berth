use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Arguments {
    #[arg(long, value_name = "FILE")]
    pub config_file: Option<PathBuf>,

    pub env_name: String,
}
