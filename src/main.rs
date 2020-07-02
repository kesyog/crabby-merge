mod bitbucket;

use anyhow::{anyhow, Result};
use bitbucket::PullRequest;
use futures::future;
use lazy_static::lazy_static;
use log::{debug, info};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

fn is_trigger_present(text: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = {
            let trigger = dotenv::var("MERGE_TRIGGER").unwrap_or_else(|_| ":shipit:".to_string());
            debug!("Trigger: \"{}\"", trigger);
            Regex::new(&format!("(?m)^{}$", trigger)).unwrap()
        };
    }
    RE.is_match(text)
}

async fn should_merge(_api: &bitbucket::Api, pr: &PullRequest) -> bool {
    is_trigger_present(pr.description.as_ref().unwrap_or(&"".to_string()))
}

async fn search_owned_prs(api: Arc<bitbucket::Api>) -> Result<()> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(2);
    params.insert("state", "open".to_string());
    params.insert("role", "author".to_string());
    let mut futures = vec![];
    for pr in api.get_prs(Some(params)).await? {
        debug!("Checking {}", pr.links["self"][0]["href"]);
        let api_shared = api.clone();
        let f = tokio::spawn(async move {
            if should_merge(&api_shared, &pr).await
                && api_shared.merge_pr(&pr).await.unwrap_or(false)
            {
                info!("merged {}", pr.links["self"][0]["href"]);
            }
        });
        futures.push(f);
    }
    tokio::join!(future::join_all(futures));
    Ok(())
}

async fn search_approved_prs(api: Arc<bitbucket::Api>) -> Result<()> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(3);
    params.insert("state", "open".to_string());
    params.insert("role", "reviewer".to_string());
    params.insert("participantStatus", "approved".to_string());
    let mut futures = vec![];
    for pr in api.get_prs(Some(params)).await? {
        debug!("Checking {}", pr.links["self"][0]["href"]);
        let api_shared = api.clone();
        let f = tokio::spawn(async move {
            if should_merge(&api_shared, &pr).await
                && api_shared.merge_pr(&pr).await.unwrap_or(false)
            {
                info!("merged {}", pr.links["self"][0]["href"]);
            }
        });
        futures.push(f);
    }
    tokio::join!(future::join_all(futures));
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    simple_logger::init_with_level(log::Level::Info)?;
    dotenv::dotenv().ok();
    let bitbucket_token = dotenv::var("BITBUCKET_API_TOKEN")
        .map_err(|_| anyhow!("No Bitbucket API token provided"))?;
    let bitbucket_url =
        dotenv::var("BITBUCKET_URL").map_err(|_| anyhow!("No Bitbucket URL provided"))?;

    let api = Arc::new(bitbucket::Api::new(&bitbucket_url, &bitbucket_token));
    let api1 = Arc::clone(&api);
    let f1 = tokio::spawn(search_owned_prs(api1));
    let api2 = Arc::clone(&api);
    let f2 = tokio::spawn(search_approved_prs(api2));
    let _ = tokio::join!(f1, f2);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn check_regex() {
        // positive tests
        assert!(is_trigger_present(":shipit:"));
        assert!(is_trigger_present(":shipit:\n"));
        assert!(is_trigger_present("\n:shipit:"));
        assert!(is_trigger_present("\n:shipit:\n"));

        // negative tests
        assert!(!is_trigger_present(":shipit: "));
        assert!(!is_trigger_present(" :shipit: "));
        assert!(!is_trigger_present(" :shipit:"));
    }
}
