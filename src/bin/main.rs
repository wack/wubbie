use __service_name__::cli::Cli;
use clap::{CommandFactory, Parser};

#[tokio::main]
async fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        None => empty_command(),
        Some(cmd) => cmd.dispatch().await,
    }
}

fn empty_command() -> miette::Result<()> {
    Cli::command()
        .print_long_help()
        .expect("unable to print help message");
    Ok(())
}
