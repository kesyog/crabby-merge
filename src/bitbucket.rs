use anyhow::Result;
use log::info;
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Response,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub id: u32,
    pub description: Option<String>,
    pub to_ref: serde_json::Value,
    pub links: serde_json::Value,
    pub version: i32,
}

#[derive(Debug, Clone)]
pub struct Api {
    base_url: String,
    http_client: reqwest::Client,
}

impl Api {
    pub fn new(base_url: &impl ToString, api_token: &impl ToString) -> Self {
        let mut headers = HeaderMap::with_capacity(3);
        let auth_header_value = "Bearer ".to_string() + &api_token.to_string();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_header_value).unwrap(),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Self {
            base_url: base_url.to_string(),
            http_client: reqwest::Client::builder()
                .default_headers(headers)
                .http1_title_case_headers()
                .build()
                .unwrap(),
        }
    }

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

    async fn get(
        &self,
        endpoint: &str,
        params: Option<&HashMap<&str, String>>,
    ) -> Result<Response> {
        let url = self.base_url.clone() + endpoint;
        Ok(self.http_client.get(&url).query(&params).send().await?)
    }

    async fn get_paged_api(
        &self,
        endpoint: &str,
        params: Option<HashMap<&str, String>>,
    ) -> Result<Vec<serde_json::Value>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Page {
            next_page_start: Option<u32>,
            // is_last_page: bool,
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

    pub async fn _get_username(&self) -> Result<String> {
        Ok(self
            .get("/plugins/servlet/applinks/whoami", None)
            .await?
            .text()
            .await?)
    }

    pub async fn get_prs(&self, params: Option<HashMap<&str, String>>) -> Result<Vec<PullRequest>> {
        let raw_result = self
            .get_paged_api("/rest/api/1.0/dashboard/pull-requests", params)
            .await?;
        Ok(serde_json::from_value(serde_json::Value::Array(
            raw_result,
        ))?)
    }

    pub async fn can_merge(&self, pr: &PullRequest) -> Result<bool> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct MergeGetResponse {
            can_merge: bool,
        }

        let endpoint = format!(
            "/rest/api/1.0/projects/{project_key}/repos/{repo_slug}/pull-requests/{id}/merge",
            project_key = pr.to_ref["repository"]["project"]["key"].as_str().unwrap(),
            repo_slug = pr.to_ref["repository"]["slug"].as_str().unwrap(),
            id = pr.id,
        );
        let response: MergeGetResponse =
            serde_json::from_str(&self.get(&endpoint, None).await?.text().await?)?;
        Ok(response.can_merge)
    }

    pub async fn merge_pr(&self, pr: &PullRequest) -> Result<bool> {
        #[derive(Serialize)]
        struct MergePostBody {
            version: i32,
        }

        if !self.can_merge(pr).await? {
            // TODO: print reason from server response
            info!("Not ready to merge: {}", pr.links["self"][0]["href"]);
            return Ok(false);
        }

        let endpoint = format!(
            "/rest/api/1.0/projects/{project_key}/repos/{repo_slug}/pull-requests/{id}/merge",
            project_key = pr.to_ref["repository"]["project"]["key"].as_str().unwrap(),
            repo_slug = pr.to_ref["repository"]["slug"].as_str().unwrap(),
            id = pr.id,
        );
        let post_body = serde_json::to_string(&MergePostBody {
            version: pr.version,
        })?;
        let response = self.post(&endpoint, None, Some(post_body)).await?;
        Ok(response.status().as_u16() == 200)
    }
}
