//! # crabby-merge
//!
//! Scans open Bitbucket pull requests for a configurable trigger string and merges them for you.
//!
//! This is mostly just a ripoff port of [polly-merge](https://github.com/noahp/polly-merge) into
//! async Rust for learning purposes.
//!
//! ## Installation
//!
//! Install via [Cargo](https://rustup.rs):
//!
//! ```sh
//! cargo install crabby-merge
//! ```
//!
//! ## Usage
//!
//! Ideally, you'd schedule crabby-merge to be run periodically. To accomplish this with [cron](https://en.wikipedia.org/wiki/Cron),
//! on a Unix-like machine, run `crontab -e` and add an entry like:
//!
//! ```text
//! # Schedule crabby-merge to run every two minutes
//! */2 * * * * $HOME/.cargo/bin/crabby-merge
//! ```
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
//!
//! ## Jenkins rebuild support
//!
//! There is experimental support for rebuilding failed Jenkins builds whose name matches a provided
//! regex trigger. This is a sad workaround for flaky blocking tests. This is compile-time gated by
//! the `jenkins` feature, which is enabled by default.
//!
//! To use it, add the following fields to your configuration file. If these fields aren't provided,
//! the retry functionality will be disabled at runtime.
//!
//! ```toml
//! jenkins_username = ""
//! jenkins_password = ""
//! # Regex trigger to search against the build name
//! jenkins_retry_trigger = ""
//! # Optional. Defaults to 10.
//! jenkins_retry_limit = ""
//! ```

use crabby_merge::bitbucket;
#[cfg(feature = "jenkins")]
use crabby_merge::history_file;
use crabby_merge::search;
use crabby_merge::Config;

use anyhow::Result;
use cfg_if::cfg_if;
use futures::future;
use log::*;
use simple_logger::SimpleLogger;
use std::sync::Arc;

#[tokio::main(flavor = "current_thread")]
#[doc(hidden)]
async fn main() -> Result<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let config = Config::load_from_default_file()?;
    let api = Arc::new(bitbucket::Client::new(
        config.bitbucket_url.clone(),
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

    cfg_if! {
        if #[cfg(feature = "jenkins")] {
            history_file::decruft().ok();
        }
    }
    info!("🚢 all done");
    Ok(())
}
