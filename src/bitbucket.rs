use anyhow::{anyhow, Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Response,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Default, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    id: u32,
    pub description: Option<String>,
    // TODO: use strong typing rather than relying on serde_json::Value to eliminate a bunch of
    // panic paths from indexing
    to_ref: serde_json::Value,
    // The PR's URL is nested within this field so it's left public
    pub links: serde_json::Value,
    version: i32,
}

#[derive(Debug, Clone)]
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

    /// Returns all of the values returned by a paged GET endpoint
    async fn get_paged_api(
        &self,
        endpoint: &str,
        params: Option<HashMap<&str, String>>,
    ) -> Result<Vec<serde_json::Value>> {
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
        Ok(values)
    }

    /// Returns the username of the authenticated user
    pub async fn _get_username(&self) -> Result<String> {
        Ok(self
            .get("/plugins/servlet/applinks/whoami", None)
            .await?
            .text()
            .await?)
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
        Ok(serde_json::from_value(serde_json::Value::Array(
            raw_result,
        ))?)
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
            .with_context(|| format!("PR not ready to merge: {}", pr.links["self"][0]["href"]))?;

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
                pr.links["self"][0]["href"],
                response.text().await?
            ))
        }
    }
}
