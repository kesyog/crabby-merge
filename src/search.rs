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

async fn should_merge(api: &bitbucket::Api, pr: &PullRequest, username: &str) -> bool {
    // Check PR description for trigger
    if is_trigger_present(pr.description.as_ref().unwrap_or(&"".to_string())) {
        info!("Found trigger in PR description");
        return true;
    }
    let comments = match api.get_pr_comments(pr, Some(username)).await {
        Ok(comments) => comments,
        Err(e) => {
            error!("{:#}", e);
            Vec::new()
        }
    };
    for comment in comments {
        if is_trigger_present(&comment) {
            info!("Found trigger in PR comment");
            return true;
        }
    }
    false
}

async fn check_prs(api: Arc<bitbucket::Api>, prs: Vec<PullRequest>, username: Arc<String>) {
    future::join_all(prs.into_iter().map(|pr| {
        debug!("Checking {}", pr.links["self"][0]["href"]);
        let api_shared = Arc::clone(&api);
        let username = Arc::clone(&username);
        async move {
            if !should_merge(&api_shared, &pr, &username).await {
                debug!("No merge trigger found in {}", pr.links["self"][0]["href"]);
                return;
            }

            match api_shared.merge_pr(&pr).await {
                Ok(()) => info!("Merged {}", pr.links["self"][0]["href"]),
                Err(e) => error!("{:#}", e),
            };
        }
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
    let username = prs[0].author["user"]["name"].as_str().unwrap().to_string();
    debug!("Username found for own PR's: {}", username);
    check_prs(api, prs, Arc::new(username)).await;
    Ok(n_prs)
}

/// Searches PR's approved by the authenticated user for the merge trigger and returns the number
/// of PR's checked.
pub async fn approved_prs(api: Arc<bitbucket::Api>) -> Result<usize> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(3);
    params.insert("state", "open".to_string());
    params.insert("role", "reviewer".to_string());
    params.insert("participantStatus", "approved".to_string());
    let (prs, username) = future::join(api.get_prs(Some(params)), api.get_username()).await;
    let prs = prs?;
    let username = username?;

    let n_prs = prs.len();
    debug!("Username returned by API: {}", username);
    check_prs(api, prs, Arc::new(username)).await;
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
