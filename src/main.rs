//! # crabby-merge
//!
//! Scans open Bitbucket pull requests for a configurable trigger string and merges them for you.
//!
//! This is mostly just a ripoff port of [polly-merge](https://github.com/noahp/polly-merge) into
//! async Rust for learning purposes.
//!
//! ## Configuration
//!
//! ### TOML
//!
//! In `$HOME/.crabby_merge.toml`:
//!
//! ```toml
//! # base URL of the Bitbucket server to query. Required.
//! bitbucket_url = "your URL goes here"
//! # API token for user authentication. Required.
//! bitbucket_api_token = "your token goes here"
//! # Trigger regex string to look for
//! merge_trigger = "^:shipit:$"
//! # Whether to check the pull request description for the trigger
//! check_description = true
//! # Whether to check pull request comments for the trigger. Only the user's own comments are searched.
//! check_comments = false
//! # Whether to include the user's own pull requests
//! check_own_prs = true
//! # Whether to search pull requests the user has approved
//! check_approved_prs = false
//! ```
//!
//! All fields are optional unless indicated. Values shown are the default values.
//!
//! ### Environment variables
//!
//! Each of the TOML keys listed above can be prefixed with `CRABBY_MERGE` and provided as an
//! environment variable. Keys are case-insensitive.
//!
//! For example, you can pass in the bitbucket API token as `CRABBY_MERGE_API_TOKEN=<your token here>`.

mod bitbucket;
mod search;

use anyhow::{anyhow, Context, Result};
use config::{Environment, File, FileFormat};
use futures::future;
use log::{error, info};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use simple_logger::SimpleLogger;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct Config {
    bitbucket_url: String,
    bitbucket_api_token: String,
    merge_trigger: String,
    check_description: bool,
    check_comments: bool,
    check_own_prs: bool,
    check_approved_prs: bool,
    // A bit awkward to keep the regex here. Problem for another day.
    #[serde(skip)]
    merge_regex: Option<Regex>,
}

fn load_config() -> Result<Config> {
    let mut config_path =
        dirs::home_dir().ok_or_else(|| anyhow!("Couldn't resolve home directory"))?;
    config_path.push(Path::new(".crabby_merge.toml"));
    let config_path = config_path
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow!("Couldn't resolve config_path"))?;

    let mut config_loader = config::Config::new();
    config_loader
        .merge(File::new(&config_path, FileFormat::Toml).required(false))?
        .merge(Environment::with_prefix("CRABBY_MERGE"))?
        .set_default("merge_trigger", ":shipit:")?
        .set_default("check_description", true)?
        .set_default("check_comments", false)?
        .set_default("check_own_prs", true)?
        .set_default("check_approved_prs", false)?;

    let mut config: Config = config_loader.try_into().with_context(|| {
        format!(
            "Please set bitbucket_url and bitbucket_api_token either in {} or as environment \
            variables with the prefix \"CRABBY_MERGE\"",
            config_path
        )
    })?;
    config.merge_regex = Some(
        RegexBuilder::new(&config.merge_trigger)
            .multi_line(true)
            .build()
            .with_context(|| format!("Bad regex: {}", config.merge_trigger))?,
    );
    Ok(config)
}

#[tokio::main]
#[doc(hidden)]
async fn main() -> Result<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let config = load_config()?;
    let api = Arc::new(bitbucket::Api::new(
        &config.bitbucket_url,
        &config.bitbucket_api_token,
    ));

    // Wrap config in an Arc to be able to pass it across async tasks
    let config = Arc::new(config);

    // Return the number of PR's checked
    let f1 = async {
        if !config.check_own_prs {
            return 0;
        }
        let api = Arc::clone(&api);
        let config = Arc::clone(&config);
        let n_prs = async move {
            match search::own_prs(api, config).await {
                Ok(n) => n,
                Err(e) => {
                    error!("{}", e);
                    0
                }
            }
        }
        .await;
        info!("Own PR's checked: {}", n_prs);
        n_prs
    };

    // Return the number of PR's checked
    let f2 = async {
        if !config.check_approved_prs {
            return 0;
        }
        let api = Arc::clone(&api);
        let config = Arc::clone(&config);
        let n_prs = async move {
            match search::approved_prs(api, config).await {
                Ok(n) => n,
                Err(e) => {
                    error!("{}", e);
                    0
                }
            }
        }
        .await;
        info!("Approved PR's checked: {}", n_prs);
        n_prs
    };

    let _ = future::join(f1, f2).await;
    info!("🚢 all done");
    Ok(())
}
