use anyhow::Context;
use futures::{Stream, TryStreamExt};
use sqlx::{ConnectOptions, QueryBuilder, SqlitePool};
use std::{env, str::FromStr};

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

        // let pool = SqlitePool::connect(&env::var("DATABASE_URL")?)
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("Failed to open DB connection");

        Ok(Self { pool })
    }

    pub async fn clear(&self) -> anyhow::Result<()> {
        let sql = sqlx::query!("DELETE FROM repos");
        sql.execute(&self.pool).await?;
        Ok(())
    }

    /// Read results that match the given filter
    pub async fn search_repositories(
        &self,
        filter: &str,
    ) -> anyhow::Result<impl Iterator<Item = String>> {
        log::debug!("filter rows with {:?}", filter);
        let filter = format!("%{}%", filter);
        let recs = sqlx::query!("SELECT name FROM repos WHERE name like ? LIMIT 5", filter) // filter
            .fetch_all(&self.pool)
            .await?;

        Ok(recs.into_iter().map(|repo| repo.name))
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
