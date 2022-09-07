#![feature(iterator_try_collect)]
mod alfred;
mod config;
mod db_client;
mod gh_client;
mod spawn_daemon;
use futures::try_join;

use crate::{
    alfred::AlfredItem, db_client::DBClient, gh_client::GHClient, spawn_daemon::DaemonResult,
};
use clap::Parser;
use futures::TryStreamExt;
use spawn_daemon::spawn_daemon;

// Parsed command instructions from the command line
#[derive(Parser)]
#[clap(author, about, version)]
struct GhAlfredCommand {
    /// pull repositories from GH and cache them into the sqlite db
    /// This is is mainly useful for testing purpose, as the update will be launched in a
    /// background daeamon process on regular basis to keep the cache up to date
    #[clap(long = "update-db")]
    update_db: bool,

    /// clear the sqlite database
    #[clap(long = "clear-db")]
    clear_db: bool,

    /// returns the first 10 Github repositories that match the filter
    #[clap(name = "filter")]
    filter: Option<String>,
}

/// process will be launched in a process daemon fork, and this currently does not play well with
/// async executors. See https://github.com/tokio-rs/tokio/issues/4301
#[tokio::main]
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
        // TODO: we could communicate updates to the main process through a unix socket here as an
        // optimisation in the future
    }

    Ok(())
}

/// Execute the clear-db command
#[tokio::main]
async fn clear_db() -> anyhow::Result<()> {
    log::info!("Clear DB");
    let db = DBClient::create().await?;
    config::GhAlfredConfig::load()?.reset_last_update_start_time()?;
    db.clear().await
}

/// Execute the search command
#[tokio::main]
async fn search_repositories(filter: String) -> anyhow::Result<()> {
    log::info!("Search DB with {:?}", filter);

    let db = DBClient::create().await?;

    let mut repositories = db.search_repositories(&filter).await?.collect::<Vec<_>>();

    // if we don't have any results we search on GH instead
    if repositories.is_empty() {
        let gh = GHClient::create().await?;
        repositories = gh.search_repositories(&filter).await?.collect();
    }

    let results = repositories
        .into_iter()
        .map(|title| AlfredItem { title })
        .collect::<Vec<_>>();

    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    // load .env file
    dotenvy::dotenv()?;

    // parse the filter string from the command line
    let args = GhAlfredCommand::parse();

    // initialize logger
    let _logger = flexi_logger::Logger::try_with_env()?
        .log_to_file(flexi_logger::FileSpec::default().suppress_timestamp())
        .start()?;

    // execute specified command and return
    if args.update_db {
        return update_db();
    } else if args.clear_db {
        return clear_db();
    }

    // When we execute the default command (filter repositories results), we will first check
    // weather or not we need to spawn a daemon in the background to populate the DB

    // read the program config
    let mut config = config::GhAlfredConfig::load()?;

    // check weather or not we should update the db in the background
    if config.should_update_db() {
        log::info!("config outdated, starting db-update daemon");
        config.update_last_update_start_time()?;
        if let DaemonResult::Daemon = spawn_daemon() {
            return update_db();
        }
    } else {
        log::info!("config up to date, no update triggered {:?}", config);
    }

    if let Some(filter) = args.filter {
        return search_repositories(filter);
    }

    Ok(())
}
