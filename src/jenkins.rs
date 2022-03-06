#![cfg(feature = "jenkins")]

use anyhow::{anyhow, Result};
use jenkins_api::{
    action::{parameters::*, ParametersAction},
    build::WorkflowRun,
};
use log::*;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::ACCEPT;

pub const DEFAULT_RETRY_LIMIT: u32 = 5;

#[derive(Debug, Clone)]
/// Authentication information
pub struct Auth {
    /// Username
    username: String,
    /// Password or API token
    password: String,
}

impl Auth {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }
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
            Lazy::new(|| Regex::new(r"^(.*)/(\d+)(?:/)?(?:display/redirect)?$").unwrap());

        let captures = URL_REGEX
            .captures(job_url)
            .ok_or_else(|| anyhow!("Invalid URL: {}", job_url))?;

        Ok(Self {
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
            } else {
                warn!("Parameter is not a String or Boolean parameter");
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

/// Attempt to rebuild the given build
#[cfg(feature = "jenkins")]
pub async fn rebuild(build_url: &str, jenkins_auth: Auth) -> Result<()> {
    let job = Job::new(build_url, jenkins_auth.clone())?;
    let client = reqwest::Client::new();
    job.rebuild(&client).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_with_suffix() {
        let auth = Auth::new(String::from("user"), String::from("hunter2"));
        let job = Job::new(
            "http://www.myjenkins.com/project/101/display/redirect",
            auth,
        )
        .unwrap();
        assert_eq!(
            "http://www.myjenkins.com/project/101/api/json",
            job.job_url()
        );
        assert_eq!(
            "http://www.myjenkins.com/project/buildWithParameters",
            job.trigger_url()
        );
    }

    #[test]
    fn url_without_suffix() {
        let auth = Auth::new(String::from("user"), String::from("hunter2"));
        let job = Job::new("http://www.myjenkins.com/project/101/", auth).unwrap();
        assert_eq!(
            "http://www.myjenkins.com/project/101/api/json",
            job.job_url()
        );
        assert_eq!(
            "http://www.myjenkins.com/project/buildWithParameters",
            job.trigger_url()
        );
    }

    #[test]
    fn url_without_suffix_no_trailing_slash() {
        let auth = Auth::new(String::from("user"), String::from("hunter2"));
        let job = Job::new("http://www.myjenkins.com/project/101", auth).unwrap();
        assert_eq!(
            "http://www.myjenkins.com/project/101/api/json",
            job.job_url()
        );
        assert_eq!(
            "http://www.myjenkins.com/project/buildWithParameters",
            job.trigger_url()
        );
    }
}
