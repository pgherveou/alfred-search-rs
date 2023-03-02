#![feature(iterator_try_collect)]
mod alfred;
mod config;
mod crate_client;
mod db_client;
mod gh_client;
mod spawn_daemon;
use crate::crate_client::CrateClient;
use crate::{
    alfred::AlfredItem, db_client::DBClient, gh_client::GHClient, spawn_daemon::DaemonResult,
};
use clap::Parser;
use futures::try_join;
use futures::TryStreamExt;
use serde::Serialize;
use spawn_daemon::spawn_daemon;

// Parsed command instructions from the command line
#[derive(Parser)]
#[clap(author, about, version)]
struct GhAlfredCommand {
    /// the command to execute
    #[clap(subcommand)]
    command: CliCommand,
}

/// The subcommand to execute
#[derive(Parser, Debug)]
enum CliCommand {
    /// Search for a github repository
    SearchGH { filter: String },
    /// Search for a rust crate
    SearchCrate { filter: String },
    /// Update the database
    /// This is is mainly useful for testing purpose, as the update will be launched in a
    /// background daeamon process on regular basis to keep the cache up to date
    UpdateDb,
    /// Clear the database
    ClearDb,
}

/// exeute the update database command
async fn update_db() -> anyhow::Result<()> {
    log::info!("Update DB");

    // get a Github and DB client
    let (gh, db) = try_join!(GHClient::create(), DBClient::create())?;

    // stream repositories
    let repositories = gh.stream_repositories();
    tokio::pin!(repositories);

    // pipe stream to save repositories into the db
    let inserts = db.save_all_repositories(repositories);
    tokio::pin!(inserts);

    // consume the pipe
    while inserts.try_next().await?.is_some() {
        log::info!("Update available");
    }

    Ok(())
}

/// exexute the clear data command
async fn clear_db() -> anyhow::Result<()> {
    log::info!("Clear DB");
    let db = DBClient::create().await?;
    config::GhAlfredConfig::load()?.reset_last_update_start_time()?;
    db.clear().await
}

/// Execute the search github repository command
async fn search_gh_repositories(filter: String) -> anyhow::Result<()> {
    let db = DBClient::create().await?;

    // search repositories in the db first
    let mut repositories = db.search_repositories(&filter).await?.collect::<Vec<_>>();

    // if we don't have any results we search on GH instead
    if repositories.is_empty() {
        let gh = GHClient::create().await?;
        repositories = gh.search_repositories(&filter).await?;
    }

    let results: Vec<AlfredItem> = repositories
        .into_iter()
        .map(|item: gh_client::GHApiRepoSearchItem| item.into())
        .collect::<Vec<_>>();

    print_results(&results)
}

/// Execute the search crate command
async fn search_crate(filter: String) -> anyhow::Result<()> {
    let db = DBClient::create().await?;

    // search repositories in the db first
    let mut crates = db.search_crates(&filter).await?.collect::<Vec<_>>();

    // if we don't have any results we search on GH instead
    if crates.is_empty() {
        let client = CrateClient::create().await?;
        crates = client.search_crate(&filter).await?;
    }

    let results: Vec<AlfredItem> = crates
        .into_iter()
        .map(|item| item.into())
        .collect::<Vec<_>>();

    print_results(&results)
}

/// Print the results as JSON to stdout
fn print_results<T: Serialize>(value: &T) -> anyhow::Result<()> {
    if cfg!(debug_assertions) {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{}", serde_json::to_string(&value)?);
    }
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    // load .env file
    dotenvy::dotenv()?;

    // parse the filter string from the command line
    let args = GhAlfredCommand::parse();

    // initialize logger
    let logger = flexi_logger::Logger::try_with_env()?;

    #[cfg(not(debug_assertions))]
    let logger = logger.log_to_file(flexi_logger::FileSpec::default().suppress_timestamp());
    logger.start()?;

    if !matches!(&args.command, CliCommand::UpdateDb | CliCommand::ClearDb) {
        run_update_daemon_if_needed()?;
    }

    run_subcommand(args.command)?;

    Ok(())
}

/// Run the update daemon if needed to warmup our local database
fn run_update_daemon_if_needed() -> Result<(), anyhow::Error> {
    // read the program config
    let mut config = config::GhAlfredConfig::load()?;

    // check weather or not we should update the db in the background
    if config.should_update_db() {
        log::info!("config outdated, starting db-update daemon");
        config.update_last_update_start_time()?;
        if let DaemonResult::Daemon = spawn_daemon() {
            return run_update_daemon_fork();
        }
    } else {
        log::info!("config up to date, no update triggered {:?}", config);
    }

    Ok(())
}

/// Execute the code that should run in the daemon fork
///
/// # Note
///
/// This is a separate function to be able to use the `#[tokio::main]` macro on it
/// Since  daemon fork does not play well with async executors. See https://github.com/tokio-rs/tokio/issues/4301#[tokio::main]
#[tokio::main]
async fn run_update_daemon_fork() -> Result<(), anyhow::Error> {
    update_db().await
}

/// Execute the parsed subcommand
#[tokio::main]
async fn run_subcommand(command: CliCommand) -> Result<(), anyhow::Error> {
    match command {
        CliCommand::UpdateDb => update_db().await,
        CliCommand::ClearDb => clear_db().await,
        CliCommand::SearchCrate { filter } => search_crate(filter).await,
        CliCommand::SearchGH { filter } => search_gh_repositories(filter).await,
    }
}
