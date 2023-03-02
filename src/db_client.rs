use anyhow::Context;
use futures::{Stream, TryStreamExt};
use sqlx::{ConnectOptions, QueryBuilder, SqlitePool};
use std::{env, str::FromStr};

use crate::{crate_client::CrateSearchItem, gh_client::GHApiRepoSearchItem};

/// Utility struct to read / write from the Sqlite database
#[derive(Clone)]
pub struct DBClient {
    pool: SqlitePool,
}

#[derive(Default, Debug, Clone)]
pub struct DBUpdateEvent;

impl DBClient {
    pub async fn create() -> anyhow::Result<Self> {
        let url = &env::var("DATABASE_URL")?;

        let mut options = sqlx::sqlite::SqliteConnectOptions::from_str(url)?;
        options.disable_statement_logging();

        let pool = SqlitePool::connect_with(options).await?;

        Ok(Self { pool })
    }

    pub async fn clear(&self) -> anyhow::Result<()> {
        let sql = sqlx::query!("DELETE FROM repos");
        sql.execute(&self.pool).await?;
        Ok(())
    }

    /// Search repositories matching the given query string
    pub async fn search_repositories(
        &self,
        filter: &str,
    ) -> anyhow::Result<impl Iterator<Item = GHApiRepoSearchItem>> {
        log::debug!("search repositories matching {filter}");
        let filter = format!("%{}%", filter);
        let recs = sqlx::query!("SELECT name FROM repos WHERE name like ? LIMIT 5", filter) // filter
            .fetch_all(&self.pool)
            .await?;

        Ok(recs.into_iter().map(|repo| GHApiRepoSearchItem {
            full_name: repo.name,
        }))
    }

    /// Search crattes matching the given query string
    pub async fn search_crates(
        &self,
        filter: &str,
    ) -> anyhow::Result<impl Iterator<Item = CrateSearchItem>> {
        log::debug!("search crates matching {filter}");
        let filter = format!("%{}%", filter);
        let recs = sqlx::query!("SELECT name FROM crates WHERE name like ? LIMIT 5", filter)
            .fetch_all(&self.pool)
            .await?;

        Ok(recs
            .into_iter()
            .map(|repo| CrateSearchItem { name: repo.name }))
    }

    /// Save the passed repositories
    async fn save_repositories(&self, repos: &[String]) -> anyhow::Result<()> {
        if repos.is_empty() {
            return Ok(());
        }

        log::info!("Insert batch starting with {}", repos[0]);
        let mut conn = self.pool.acquire().await?;
        let mut query_builder: QueryBuilder<sqlx::Sqlite> =
            QueryBuilder::new("INSERT OR REPLACE INTO repos(name) ");

        query_builder.push_values(repos.iter(), |mut b, repo| {
            b.push_bind(repo);
        });

        let query = query_builder.build();

        query.execute(&mut conn).await?;
        Ok(())
    }

    pub fn save_all_repositories<'a>(
        &'a self,
        mut src: impl Stream<Item = anyhow::Result<Vec<String>>> + std::marker::Unpin + 'a,
    ) -> impl Stream<Item = anyhow::Result<DBUpdateEvent>> + 'a {
        async_stream::try_stream!({
            while let Some(repos) = src.try_next().await? {
                self.save_repositories(&repos)
                    .await
                    .context("failed to save repositories")?;
                yield DBUpdateEvent {};
            }
        })
    }
}
