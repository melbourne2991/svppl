use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use inquire::Text;
use crate::project_template;

/// A fictional versioning CLI
#[derive(Debug, Parser)] // requires `derive` feature
#[command(name = "svppl")]
#[command(about = "Tasks, tasks, tasks.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initializes a new project
    Init {

    },

    /// Initializes a new project
    Deploy {},
}

pub fn start() {
    let args = Cli::parse();

    match args.command {
        Commands::Init {} => {
            init_project();
        }
        Commands::Deploy {} => {
            println!("Deploy");
        }
    }
}

fn init_project() {
    Text::new("Project name").prompt();
}