use anyhow::{anyhow, Result};
use jenkins_api::{
    action::{parameters::*, ParametersAction},
    build::WorkflowRun,
};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::ACCEPT;

#[derive(Debug, Clone)]
/// Authentication information
pub struct Auth {
    /// Username
    pub username: String,
    /// Password or API token
    pub password: String,
}

#[derive(Debug, Clone)]
/// A Bitbucket API client
pub struct Job {
    base_url: String,
    /// Numerical job id
    id: String,
    credentials: Auth,
}

impl Job {
    pub fn new(job_url: &str, credentials: Auth) -> Result<Self> {
        static URL_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^(.*/)(\d+)(?:/display/redirect)?$").unwrap());

        let captures = URL_REGEX
            .captures(job_url)
            .ok_or_else(|| anyhow!("Invalid URL: {}", job_url))?;

        Ok(Job {
            base_url: captures[1].to_string(),
            id: captures[2].to_string(),
            credentials,
        })
    }

    fn job_url(&self) -> String {
        format!("{}/{}/api/json", self.base_url, self.id)
    }

    fn trigger_url(&self) -> String {
        format!("{}/buildWithParameters", self.base_url)
    }

    async fn fetch_build(&self, client: &reqwest::Client) -> Result<WorkflowRun> {
        Ok(client
            .get(&self.job_url())
            .header(ACCEPT, "application/json")
            .basic_auth(&self.credentials.username, Some(&self.credentials.password))
            .send()
            .await?
            .json()
            .await?)
    }

    /// Trigger a rebuild of the Jenkins job represented by `self`
    pub async fn rebuild(&self, client: &reqwest::Client) -> Result<()> {
        let build = self.fetch_build(client).await?;
        let build_parameters = build
            .actions
            .iter()
            .find_map(|action| action.as_variant::<ParametersAction>().ok())
            .ok_or_else(|| anyhow!("Could not find build parameters"))?
            .parameters;

        let mut request = client.post(&self.trigger_url());
        for param in build_parameters {
            // Assume all build parameters are either string or boolean parameters
            if let Ok(param) = param.as_variant::<StringParameterValue>() {
                request = request.query(&[(&param.name, &param.value)]);
            } else if let Ok(param) = param.as_variant::<BooleanParameterValue>() {
                request = request.query(&[(&param.name, &param.value.to_string())]);
            }
        }
        let response = request
            .basic_auth(&self.credentials.username, Some(&self.credentials.password))
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(anyhow!("Rebuild returned {}", response.status()))
        }
    }
}