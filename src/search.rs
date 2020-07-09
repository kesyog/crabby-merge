use crate::bitbucket::PullRequest;
use crate::{bitbucket, Config};
use anyhow::Result;
use futures::future;
use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::Arc;

async fn should_merge(
    api: &bitbucket::Api,
    pr: &PullRequest,
    username: &str,
    config: Arc<Config>,
) -> bool {
    if config.check_description
        && config
            .merge_regex
            .as_ref()
            .map(|re| re.is_match(pr.description.as_ref().unwrap_or(&"".to_string())))
            .unwrap()
    {
        info!("Found trigger in PR description");
        return true;
    }
    if config.check_comments {
        let comments = match api.get_pr_comments(pr, Some(username)).await {
            Ok(comments) => comments,
            Err(e) => {
                error!("{:#}", e);
                Vec::new()
            }
        };
        for comment in comments {
            if config
                .merge_regex
                .as_ref()
                .map(|re| re.is_match(&comment))
                .unwrap()
            {
                info!("Found trigger in PR comment");
                return true;
            }
        }
    }
    false
}

async fn check_prs(
    api: Arc<bitbucket::Api>,
    prs: Vec<PullRequest>,
    username: Arc<String>,
    config: Arc<Config>,
) {
    future::join_all(prs.into_iter().map(|pr| {
        debug!("Checking {}", pr.url().unwrap());
        let api_shared = Arc::clone(&api);
        let username = Arc::clone(&username);
        let config = Arc::clone(&config);
        async move {
            if !should_merge(&api_shared, &pr, &username, config).await {
                debug!("No merge trigger found in {}", pr.url().unwrap());
                return;
            }

            match api_shared.merge_pr(&pr).await {
                Ok(()) => info!("Merged {}", pr.url().unwrap()),
                Err(e) => error!("{:#}", e),
            };
        }
    }))
    .await;
}

/// Search PR's authored by the authenticated user for the merge trigger and returns the number of
/// PR's checked.
pub async fn own_prs(api: Arc<bitbucket::Api>, config: Arc<Config>) -> Result<usize> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(2);
    params.insert("state", "open".to_string());
    params.insert("role", "author".to_string());
    info!("Fetching list of own PR's");
    let prs = api.get_prs(Some(params)).await?;
    let n_prs = prs.len();
    let username = prs[0].author().expect("No author field").to_string();
    info!("Scanning {}'s PR's", username);
    check_prs(api, prs, Arc::new(username), config).await;
    Ok(n_prs)
}

/// Searches PR's approved by the authenticated user for the merge trigger and returns the number
/// of PR's checked.
pub async fn approved_prs(api: Arc<bitbucket::Api>, config: Arc<Config>) -> Result<usize> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(3);
    params.insert("state", "open".to_string());
    params.insert("role", "reviewer".to_string());
    params.insert("participantStatus", "approved".to_string());
    info!("Fetching approved PR's");
    let (prs, username) = future::join(api.get_prs(Some(params)), api.get_username()).await;
    let prs = prs?;
    let username = username?;

    let n_prs = prs.len();
    info!("Scanning PR's approved by {}", username);
    check_prs(api, prs, Arc::new(username), config).await;
    Ok(n_prs)
}
