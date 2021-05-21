use anyhow::{anyhow, Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Response,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::mem;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    id: u32,
    pub description: Option<String>,
    to_ref: serde_json::Value,
    /// `links["self"][0]["href"]` contains the PR URL
    links: serde_json::Value,
    version: i32,
    /// `author["user"]["name"]` contains the author's username
    author: serde_json::Value,
}

impl PullRequest {
    pub fn url(&self) -> Option<&str> {
        self.links.get("self").and_then(|s| {
            s.get(0)
                .and_then(|arr| arr.get("href").and_then(serde_json::Value::as_str))
        })
    }

    pub fn author(&self) -> Option<&str> {
        self.author
            .get("user")
            .and_then(|u| u.get("name").and_then(serde_json::Value::as_str))
    }
}

#[derive(Debug)]
/// A Bitbucket API client
pub struct Api {
    base_url: String,
    http_client: reqwest::Client,
}

impl Api {
    /// Returns a Bitbucket API client
    ///
    /// # Arguments
    ///
    /// * `base_url` - base URL of the Bitbucket server to query
    /// * `api_token` - API token for user authentication
    pub fn new(base_url: &impl ToString, api_token: &impl ToString) -> Self {
        let mut headers = HeaderMap::with_capacity(3);
        let auth_header_value = "Bearer ".to_string() + &api_token.to_string();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_header_value).unwrap(),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        // Maybe shouldn't send CONTENT_TYPE header for GET requests but doesn't seem to hurt
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Self {
            base_url: base_url.to_string(),
            http_client: reqwest::Client::builder()
                .default_headers(headers)
                // Bitbucket server oddly seems to require this
                .http1_title_case_headers()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
        }
    }

    /// Performs a POST request
    async fn post<T>(
        &self,
        endpoint: &str,
        params: Option<&HashMap<&str, String>>,
        body: Option<T>,
    ) -> Result<Response>
    where
        T: Into<reqwest::Body> + std::default::Default + Send,
    {
        let url = self.base_url.clone() + endpoint;
        Ok(self
            .http_client
            .post(&url)
            .query(&params)
            .body(body.unwrap_or_default())
            .send()
            .await?)
    }

    /// Performs a GET request
    async fn get(
        &self,
        endpoint: &str,
        params: Option<&HashMap<&str, String>>,
    ) -> Result<Response> {
        let url = self.base_url.clone() + endpoint;
        Ok(self.http_client.get(&url).query(&params).send().await?)
    }

    /// Returns the values returned by a paged GET endpoint
    async fn get_paged_api(
        &self,
        endpoint: &str,
        params: Option<HashMap<&str, String>>,
    ) -> Result<serde_json::Value> {
        /// The response to a single GET request
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Page {
            next_page_start: Option<u32>,
            values: Vec<serde_json::Value>,
        }

        let mut values = Vec::new();
        let mut params = params.unwrap_or_else(|| HashMap::with_capacity(1));
        params.insert("start", "0".to_string());
        loop {
            let page: Page =
                serde_json::from_str(&self.get(endpoint, Some(&params)).await?.text().await?)?;
            values.extend_from_slice(&page.values);
            if let Some(start) = page.next_page_start {
                let start_str = start.to_string();
                params.insert("start", start_str);
                continue;
            }
            break;
        }
        Ok(values.into())
    }

    /// Returns the username of the authenticated user
    pub async fn get_username(&self) -> Result<String> {
        Ok(self
            .get("/plugins/servlet/applinks/whoami", None)
            .await?
            .text()
            .await?)
    }

    /// Returns the text of all comments made on a given PR
    ///
    /// # Arguments
    ///
    /// * `pr` - Pull request to search
    /// * `username` - If not `None`, only comments written by the provided user will be
    /// included
    pub async fn get_pr_comments(
        &self,
        pr: &PullRequest,
        username: Option<&str>,
    ) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct CommentAuthor {
            name: String,
        }

        #[derive(Deserialize)]
        struct Comment {
            author: CommentAuthor,
            text: String,
            #[serde(rename = "comments")]
            replies: Vec<Comment>,
        }

        #[derive(Deserialize)]
        struct Activity {
            action: String,
            comment: Option<Comment>,
        }

        /// Helper function to recurse through comment replies
        fn recurse_nested_comments(
            comments: &mut Vec<Comment>,
            comment_text: &mut Vec<String>,
            username: &Option<&str>,
        ) {
            for comment in comments {
                if username.is_none() || *username == Some(&comment.author.name) {
                    let new_comment = mem::take(&mut comment.text);
                    comment_text.push(new_comment);
                }
                recurse_nested_comments(&mut comment.replies, comment_text, username);
            }
        }

        // Using the pull request activities API to fetch comments, as it's more ergonomic than the
        // comments API
        let endpoint = format!(
            "/rest/api/1.0/projects/{project_key}/repos/{repo_slug}/pull-requests/{id}/activities",
            project_key = pr.to_ref["repository"]["project"]["key"].as_str().unwrap(),
            repo_slug = pr.to_ref["repository"]["slug"].as_str().unwrap(),
            id = pr.id,
        );
        let activities: Vec<Activity> =
            serde_json::from_value(self.get_paged_api(&endpoint, None).await?)?;
        Ok(activities
            .into_iter()
            // Assemble a vector containing all top-level comments and comment replies, filtering
            // out comments written by other users if a username was provided
            .flat_map(|activity| {
                // The activities API can return other events besides comments. Filter out anything that
                // is not a comment.
                if activity.action != "COMMENTED" || activity.comment.is_none() {
                    return Vec::new();
                }
                let mut top_level_comment = activity.comment.unwrap();
                let mut comment_text = Vec::new();
                if username.is_none() || username == Some(&top_level_comment.author.name) {
                    let new_comment = mem::take(&mut top_level_comment.text);
                    comment_text.push(new_comment);
                }
                recurse_nested_comments(
                    &mut top_level_comment.replies,
                    &mut comment_text,
                    &username,
                );
                comment_text
            })
            .collect())
    }

    /// Returns a list of pull requests affiliated with the authenticated user
    ///
    /// # Arguments
    ///
    /// * `params` - A list of parameters to pass to the Bitbucket
    /// `/rest/api/1.0/dashboard/pull-requests` endpoint. See Bitbucket API documentation for
    /// available options.
    pub async fn get_prs(&self, params: Option<HashMap<&str, String>>) -> Result<Vec<PullRequest>> {
        let raw_result = self
            .get_paged_api("/rest/api/1.0/dashboard/pull-requests", params)
            .await?;
        Ok(serde_json::from_value(raw_result)?)
    }

    /// Check if a pull request is able to be merged without actually merging it
    pub async fn can_merge(&self, pr: &PullRequest) -> Result<()> {
        let endpoint = format!(
            "/rest/api/1.0/projects/{project_key}/repos/{repo_slug}/pull-requests/{id}/merge",
            project_key = pr.to_ref["repository"]["project"]["key"].as_str().unwrap(),
            repo_slug = pr.to_ref["repository"]["slug"].as_str().unwrap(),
            id = pr.id,
        );
        let response_text = self.get(&endpoint, None).await?.text().await?;
        let response_json: serde_json::Value = serde_json::from_str(&response_text)?;
        response_json
            .as_object()
            .and_then(|response| {
                if let Some(serde_json::Value::Bool(true)) = response.get("canMerge") {
                    Some(())
                } else {
                    None
                }
            })
            .ok_or(anyhow!(response_text))
    }

    /// Merge the given pull request
    pub async fn merge_pr(&self, pr: &PullRequest) -> Result<()> {
        // Check if the PR is blocked from merging e.g. because there's a build in progress
        // TODO: maybe just skip this check and use the POST error response instead
        self.can_merge(pr)
            .await
            .with_context(|| format!("PR not ready to merge: {}", pr.url().unwrap()))?;

        let endpoint = format!(
            "/rest/api/1.0/projects/{project_key}/repos/{repo_slug}/pull-requests/{id}/merge",
            project_key = pr.to_ref["repository"]["project"]["key"].as_str().unwrap(),
            repo_slug = pr.to_ref["repository"]["slug"].as_str().unwrap(),
            id = pr.id,
        );
        // Create json body by hand. It's just one "version" field that contains the PR version id
        let post_body = String::from(r#"{"version":"#) + &pr.version.to_string() + "}";
        let response = self.post(&endpoint, None, Some(post_body)).await?;
        if response.status().as_u16() == 200 {
            Ok(())
        } else {
            Err(anyhow!(
                "PR merge failed for {}\n{}",
                pr.url().unwrap(),
                response.text().await?
            ))
        }
    }
}
