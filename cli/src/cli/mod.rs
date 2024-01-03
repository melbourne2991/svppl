use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::project_template;
use inquire::Text;

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
    Init {},

    /// Cluster management
    Cluster {
        #[command(subcommand)]
        command: ClusterCommands,
    },
}

#[derive(Debug, Subcommand)]
enum ClusterCommands {
    /// Starts a node and joins the given cluster
    Join {

    }
}

pub async fn start() {
    let args = Cli::parse();

    let result = match args.command {
        Commands::Init {} => init_project().await,
        Commands::Cluster { command } => match command {
            ClusterCommands::Join {  } => {
                println!("Starting cluster");
                Ok(())
            }
        }
    };

    if let Err(e) = result {
        println!("Error: {:?}", e);
    }
}

async fn init_project() -> Result<()> {
    let project_name_input = Text::new("Project name:")
        .with_help_message("Leave blank to use the current directory")
        .prompt()?;

    let cwd = env::current_dir()?;

    let (project_name, target_path) = if project_name_input.len() == 0 {
        let project_name = cwd
            .file_name()
            .ok_or(anyhow!("Could not get the file name for directory"))?
            .to_str()
            .ok_or(anyhow!("Could not convert file name to string"))?;

        (project_name, cwd.clone())
    } else {
        let target = Path::new(&cwd).join(&project_name_input);
        std::fs::create_dir(&target)?;
        (project_name_input.as_str(), target)
    };

    let src = project_template::DefaultProjectTemplateSource::new();

    project_template::render(
        &src,
        project_template::ProjectTemplateOptions {
            target_dir: &target_path,
            package_name: project_name,
        },
    )
    .await?;

    Ok(())
}

async fn deploy_project() -> Result<()> {
    Ok(())
}
