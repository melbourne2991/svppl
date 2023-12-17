mod cli;
mod project_template;

#[tokio::main]

async fn main() {
    cli::start().await;
}

// use std::ffi::OsStr;
// use std::ffi::OsString;
// use std::path::PathBuf;

// use clap::{Args, Parser, Subcommand, ValueEnum};


// mod project_template;

// /// A fictional versioning CLI
// #[derive(Debug, Parser)] // requires `derive` feature
// #[command(name = "svppl")]
// #[command(about = "Tasks, tasks, tasks.", long_about = None)]
// struct Cli {
//     #[command(subcommand)]
//     command: Commands,
// }

// #[derive(Debug, Subcommand)]
// enum Commands {
//     /// Initializes a new project
//     Init {

//     },

//     /// Initializes a new project
//     Deploy {},
// }

// fn main() {
//     let args = Cli::parse();

//     match args.command {
//         Commands::Init {} => {

//         }
//         Commands::Deploy {} => {
//             println!("Deploy");
//         }
//     }
// }

// fn init() {

// }