//! Client to query the crates.io API
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::Deserialize;

/// A crate.io client
pub struct CrateClient {
    client: reqwest::Client,
}

/// response from the crates.io search API
#[derive(Deserialize)]
struct CrateSearchResponse {
    crates: Vec<CrateSearchItem>,
}

/// response item from the crates.io search API
#[derive(Deserialize)]
pub struct CrateSearchItem {
    pub name: String,
}

impl CrateClient {
    /// create a new crates.io client
    pub async fn create() -> anyhow::Result<Self> {
        let default_headers =
            HeaderMap::from_iter([(header::ACCEPT, HeaderValue::from_static("application/json"))]);

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .user_agent(env!("CARGO_PKG_NAME"))
            .build()?;

        Ok(Self { client })
    }

    /// search for crates matching the given filter
    pub async fn search_crate(&self, filter: &str) -> anyhow::Result<Vec<CrateSearchItem>> {
        log::info!("querying crates.io crate matching {filter}");
        let response = self
            .client
            .get("https://crates.io/api/v1/crates")
            .query(&[("page", "1"), ("per_page", "5"), ("q", filter)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::format_err!(
                "Failed to search crate: {}, {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let crates = response.json::<CrateSearchResponse>().await?.crates;

        Ok(crates)
    }
}
