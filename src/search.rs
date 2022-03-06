#[cfg(feature = "jenkins")]
use crate::backoff;
#[cfg(feature = "jenkins")]
use crate::bitbucket::BuildState;
use crate::bitbucket::{self, PullRequest};
#[cfg(feature = "jenkins")]
use crate::jenkins;
use crate::Config;
#[cfg(feature = "jenkins")]
use crate::History;

use anyhow::Result;
use cfg_if::cfg_if;
use futures::future;
#[cfg(feature = "jenkins")]
use guard::guard;
use log::*;
use std::collections::HashMap;
use std::sync::Arc;

async fn should_merge(
    api: &bitbucket::Client,
    pr: &PullRequest,
    username: &str,
    config: &Config,
) -> bool {
    if config.check_description
        && config
            .merge_regex
            .is_match(pr.description.as_ref().unwrap_or(&String::new()))
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
            if config.merge_regex.is_match(&comment) {
                info!("Found trigger in PR comment");
                return true;
            }
        }
    }
    false
}

/// Check PR's for merge trigger and perform configured actions
async fn check_prs(
    api: Arc<bitbucket::Client>,
    prs: Vec<PullRequest>,
    username: Arc<str>,
    config: Arc<Config>,
) {
    future::join_all(prs.into_iter().map(|pr| {
        debug!("Checking {}", pr.url().unwrap());
        let api_shared = Arc::clone(&api);
        let username = Arc::clone(&username);
        let config = Arc::clone(&config);
        tokio::spawn(async move {
            if !should_merge(&api_shared, &pr, &username, &config).await {
                debug!("No merge trigger found in {}", pr.url().unwrap());
                return;
            }

            match api_shared.merge_pr(&pr).await {
                Ok(()) => {
                    info!("Merged {}", pr.url().unwrap());
                    cfg_if! {
                        if #[cfg(feature = "jenkins")] {
                            if let Some(hash) = pr.hash() {
                                History::delete(hash).ok();
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Could not merge: {:#}", e);
                    cfg_if! {
                        if #[cfg(feature = "jenkins")] {
                            retry_pr_builds(&api_shared, &pr, &config).await;
                        }
                    }
                }
            };
        })
    }))
    .await;
}

/// Attempt to rebuild any PR builds that match the retry regex trigger
#[cfg(feature = "jenkins")]
async fn retry_pr_builds(api: &bitbucket::Client, pr: &PullRequest, config: &Config) {
    guard!(
        let (Some(jenkins_auth), Some(retry_trigger)) =
            (config.jenkins_auth.as_ref(), config.jenkins_retry_regex.as_ref())
        else {
            warn!("Jenkins not configured. Skipping retry attempt.");
            return;
        }
    );
    guard!(
        let Some(hash) = pr.hash()
        else {
            error!("Could not resolve commit hash for PR {:?}", pr);
            return;
        }
    );
    let builds = api.get_build_status(hash).await;
    for build in builds.into_iter().flatten() {
        if build.state == BuildState::Failed
            && retry_trigger.is_match(&build.name)
            && backoff::should_retry_now(hash, config.jenkins_retry_limit)
        {
            info!("Attempting rebuild for {}", build.name);
            match jenkins::rebuild(&build.url, jenkins_auth.clone()).await {
                Ok(_) => info!("Rebuilt {}", build.name),
                Err(e) => error!("{:#}", e),
            };
        }
    }
}

/// Search PR's authored by the authenticated user for the merge trigger and returns the number of
/// PR's checked.
pub async fn own_prs(api: Arc<bitbucket::Client>, config: Arc<Config>) -> Result<usize> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(2);
    params.insert("state", "open".to_owned());
    params.insert("role", "author".to_owned());
    info!("Fetching list of own PR's");
    let prs = api.get_prs(Some(params)).await?;
    let n_prs = prs.len();
    let username = Arc::from(prs[0].author().expect("No author field"));
    info!("Scanning {}'s PR's", username);
    check_prs(api, prs, username, config).await;
    Ok(n_prs)
}

/// Searches PR's approved by the authenticated user for the merge trigger and returns the number
/// of PR's checked.
pub async fn approved_prs(api: Arc<bitbucket::Client>, config: Arc<Config>) -> Result<usize> {
    let mut params: HashMap<&str, String> = HashMap::with_capacity(3);
    params.insert("state", "open".to_owned());
    params.insert("role", "reviewer".to_owned());
    params.insert("participantStatus", "approved".to_owned());
    info!("Fetching approved PR's");
    let (prs, username) = future::join(api.get_prs(Some(params)), api.get_username()).await;
    let prs = prs?;
    let username = Arc::from(username?);

    let n_prs = prs.len();
    info!("Scanning PR's approved by {}", username);
    check_prs(api, prs, username, config).await;
    Ok(n_prs)
}
