//! Github client used to query Github api
use std::time::Duration;

use crate::gh_client::repo_view::RepoViewRateLimit;
use anyhow::Context;
use chrono::Utc;
use graphql_client::{reqwest::post_graphql, GraphQLQuery};
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::Deserialize;
use tokio_stream::Stream;

/// Paged GraphQLQuery to fetch all repositories associated with the logged-in user
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "./schema.graphql",
    query_path = "./query.graphql",
    response_derives = "Debug"
)]
struct RepoView;

/// DateTime type alias referenced by the graphql macro when parsing the GH graphql schema
type DateTime = String;

/// Utility class to read from GH api
#[derive(Clone)]
pub struct GHClient {
    client: reqwest::Client,
}

/// Results extracted from the graphql query to cache all repositories related to the user
#[derive(Debug)]
struct RepoPageRead {
    /// list of repositories fetched from the API
    repos: Vec<String>,
    /// cursor used to query the next page
    end_cursor: Option<String>,
    /// delay imposed by the rate limited GH api before we can fire the next page read
    delay: Option<Duration>,
}

/// Response from the Github search API to find repositories matching our search
#[derive(Deserialize)]
struct GHApiRepoSearchResponse {
    items: Vec<GHApiRepoSearchItem>,
}

/// A single repository item returned by the Github search API
/// see [API doc](https://docs.github.com/en/rest/search#search-repositories)
/// to parse more fields returned by the API
#[derive(Deserialize)]
pub struct GHApiRepoSearchItem {
    pub full_name: String,
}

impl GHClient {
    /// Create a new Github client, using the GITHUB_API_TOKEN environment variable to authorize
    /// API calls
    pub async fn create() -> anyhow::Result<Self> {
        let token = &std::env::var("GITHUB_API_TOKEN")?;

        let default_headers = HeaderMap::from_iter([
            (
                header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token))?,
            ),
            (
                header::ACCEPT,
                HeaderValue::from_static("application/vnd.github+json"),
            ),
        ]);

        let client = reqwest::Client::builder()
            .user_agent("graphql-rust/0.10.0")
            .default_headers(default_headers)
            .build()?;

        Ok(Self { client })
    }

    /// Search repositories matching the given query string
    pub async fn search_repositories(
        &self,
        query: &str,
    ) -> anyhow::Result<Vec<GHApiRepoSearchItem>> {
        log::info!("querying api.github.com for repos matching {query}");
        let items = self
            .client
            .get("https://api.github.com/search/repositories")
            .query(&[
                ("sort", "stars"),
                ("per_page", "5"),
                ("order", "desc"),
                ("q", query),
            ])
            .send()
            .await?
            .json::<GHApiRepoSearchResponse>()
            .await?
            .items;

        Ok(items)
    }

    /// fetch one page of result from the repositories graphlql query, starting after the given
    /// `after` cursor
    async fn fetch_repositories(&self, after: Option<String>) -> anyhow::Result<RepoPageRead> {
        let variables = repo_view::Variables { after };
        let response_body =
            post_graphql::<RepoView, _>(&self.client, "https://api.github.com/graphql", variables)
                .await?;

        let data = response_body
            .data
            .ok_or_else(|| anyhow::format_err!("Missing data"))?;

        // extracts repos from response body
        let repos = data
            .viewer
            .repositories
            .nodes
            .ok_or_else(|| anyhow::format_err!("missing nodes data from response"))?
            .into_iter()
            .map(|node| {
                node.map(|n| n.name_with_owner)
                    .ok_or_else(|| anyhow::format_err!("missing name_with_owner field"))
            })
            .try_collect::<Vec<_>>()?;

        // extracts rate limit parameters
        let RepoViewRateLimit {
            remaining,
            reset_at,
            cost,
            ..
        } = data
            .rate_limit
            .ok_or_else(|| anyhow::format_err!("Missing rate_limit"))?;

        // extract end cursor
        let end_cursor = data.viewer.repositories.page_info.end_cursor;

        // calculate delay for next API call
        let delay = if remaining - cost > 0 {
            None
        } else {
            let reset_at = chrono::DateTime::parse_from_rfc3339(&reset_at)?.naive_utc();
            let delay = reset_at - Utc::now().naive_utc();
            delay.to_std().map(Some).unwrap_or(None)
        };

        Ok(RepoPageRead {
            repos,
            end_cursor,
            delay,
        })
    }

    /// Stream all repositories using the GraphQLQuery stored in query.graphql
    pub fn stream_repositories(&self) -> impl Stream<Item = anyhow::Result<Vec<String>>> + '_ {
        log::info!("start streaming repositories");
        async_stream::try_stream!({
            let mut after = None;
            loop {
                let RepoPageRead {
                    repos,
                    end_cursor,
                    delay,
                } = self
                    .fetch_repositories(after)
                    .await
                    .context("failed to fetch repository")?;

                yield repos;

                if end_cursor.is_none() {
                    break;
                }

                after = end_cursor;

                if let Some(duration) = delay {
                    log::info!(
                        "Rate Limit: Wait {:?} before making next GH api call",
                        duration
                    );
                    tokio::time::sleep(duration).await;
                }
            }
        })
    }
}
