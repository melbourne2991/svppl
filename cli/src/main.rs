mod cli;
mod project_template;

#[tokio::main]

async fn main() {
    cli::start().await;
}