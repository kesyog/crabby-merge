use crate::bitbucket;
use crate::bitbucket::PullRequest;
use anyhow::Result;
use futures::future;
use lazy_static::lazy_static;
use log::{debug, error, info};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

/// Returns whether the given string contains the merge trigger
// TODO: pass this in as an input
fn is_trigger_present(text: &str) -> bool {
    lazy_static! {
        static ref RE: Regex = {
            let trigger =
                dotenv::var("CRABBY_MERGE_TRIGGER").unwrap_or_else(|_| ":shipit:".to_string());
            debug!("Trigger: \"{}\"", &trigger);
            Regex::new(&format!("(?m)^{}$", trigger)).expect("Bad regex")
        };
    }
    RE.is_match(text)
}

async fn should_merge(_api: &bitbucket::Api, pr: &PullRequest) -> bool {
    // Check PR description for trigger
    is_trigger_present(pr.description.as_ref().unwrap_or(&"".to_string()))
    // TODO: check PR comments too
}

async fn check_prs(api: Arc<bitbucket::Api>, prs: Vec<PullRequest>) {
    future::join_all(prs.into_iter().map(|pr| {
        debug!("Checking {}", pr.links["self"][0]["href"]);
        let api_shared = Arc::clone(&api);
        tokio::spawn(async move {
            if !should_merge(&api_shared, &pr).await {
                debug!("No merge trigger found in {}", pr.links["self"][0]["href"]);
                return;
            }

            match api_shared.merge_pr(&pr).await {
                Ok(()) => info!("Merged {}", pr.links["self"][0]["href"]),
                Err(e) => error!("{:#}", e),
            };
        })
    }))
    .await;
}

/// Search PR's authored by the authenticated user for the merge trigger and returns the number of
/// PR's checked.
pub async fn own_prs(api: Arc<bitbucket::Api>) -> Result<usize> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(2);
    params.insert("state", "open".to_string());
    params.insert("role", "author".to_string());
    let prs = api.get_prs(Some(params)).await?;
    let n_prs = prs.len();
    check_prs(api, prs).await;
    Ok(n_prs)
}

/// Searches PR's approved by the authenticated user for the merge trigger and returns the number
/// of PR's checked.
pub async fn approved_prs(api: Arc<bitbucket::Api>) -> Result<usize> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(3);
    params.insert("state", "open".to_string());
    params.insert("role", "reviewer".to_string());
    params.insert("participantStatus", "approved".to_string());
    let prs = api.get_prs(Some(params)).await?;
    let n_prs = prs.len();
    check_prs(api, prs).await;
    Ok(n_prs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trigger_is_present() {
        assert!(is_trigger_present(":shipit:"));
        assert!(is_trigger_present(":shipit:\n"));
        assert!(is_trigger_present("\n:shipit:"));
        assert!(is_trigger_present("\n:shipit:\n"));
    }

    #[test]
    fn trigger_not_present() {
        assert!(!is_trigger_present(""));
        assert!(!is_trigger_present(" "));
        assert!(!is_trigger_present(" \n"));
        assert!(!is_trigger_present("\n"));
        assert!(!is_trigger_present("\n "));
        assert!(!is_trigger_present("hello"));
        assert!(!is_trigger_present("hello world"));
        assert!(!is_trigger_present(" :shipit: "));
        assert!(!is_trigger_present(" :shipit:"));
        assert!(!is_trigger_present(":shipit: "));
    }
}
