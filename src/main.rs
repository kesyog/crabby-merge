//! # crabby-merge
//!
//! Merge Bitbucket pull requests if they contain a trigger string. Only pull requests authored or
//! approved by the authenticated user are searched.
//!
//! This is mostly just a ripoff port of [polly-merge](https://github.com/noahp/polly-merge) into
//! async Rust for learning purposes.
//!
//! ### Configuration
//!
//! Set a few environment variables:
//! * `BITBUCKET_URL` - base URL of the Bitbucket server to query
//! * `BITBUCKET_API_TOKEN` - API token for user authentication
//! * `CRABBY_MERGE_TRIGGER` - (optional) Regex string to look in PR descriptions for that indicates
//! that a PR is ready to merge. Must be on its own line in the PR description. Defaults to
//! `:shipit:`.

mod bitbucket;
mod search;

use anyhow::{anyhow, Result};
use futures::future;
use log::{error, info};
use std::sync::Arc;

#[tokio::main]
#[doc(hidden)]
async fn main() -> Result<()> {
    simple_logger::init_with_level(log::Level::Info)?;
    dotenv::dotenv().ok();
    let bitbucket_token = dotenv::var("BITBUCKET_API_TOKEN")
        .map_err(|_| anyhow!("Must set BITBUCKET_API_TOKEN environment variable"))?;
    let bitbucket_url = dotenv::var("BITBUCKET_URL")
        .map_err(|_| anyhow!("Must set BITBUCKET_URL environment variable"))?;

    let api = Arc::new(bitbucket::Api::new(&bitbucket_url, &bitbucket_token));

    let api1 = Arc::clone(&api);
    // Return the number of PR's checked
    let f1 = tokio::spawn(async move {
        match search::own_prs(api1).await {
            Ok(n) => n,
            Err(e) => {
                error!("{}", e);
                0
            }
        }
    });

    let api2 = Arc::clone(&api);
    // Return the number of PR's checked
    let f2 = tokio::spawn(async move {
        match search::approved_prs(api2).await {
            Ok(n) => n,
            Err(e) => {
                error!("{}", e);
                0
            }
        }
    });

    let (own_prs_checked, approved_prs_checked) = future::join(f1, f2).await;
    info!(
        "Own PR's checked: {} Approved PR's checked: {}",
        own_prs_checked?, approved_prs_checked?
    );
    Ok(())
}
