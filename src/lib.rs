pub mod bitbucket;
#[cfg(feature = "jenkins")]
pub mod jenkins;
pub mod search;

use anyhow::{anyhow, Context, Result};
use config::{Environment, File, FileFormat};
use regex::Regex;
use regex::RegexBuilder;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    pub bitbucket_url: String,
    pub bitbucket_api_token: String,
    #[cfg(feature = "jenkins")]
    pub jenkins_auth: Option<jenkins::Auth>,
    #[cfg(feature = "jenkins")]
    pub jenkins_retry_regex: Option<Regex>,
    pub check_description: bool,
    pub check_comments: bool,
    pub check_own_prs: bool,
    pub check_approved_prs: bool,
    pub merge_regex: Regex,
}

impl Config {
    pub fn load_from_default_file() -> Result<Self> {
        #[derive(Debug, Deserialize)]
        struct Options {
            bitbucket_url: String,
            bitbucket_api_token: String,
            #[cfg(feature = "jenkins")]
            jenkins_username: Option<String>,
            #[cfg(feature = "jenkins")]
            jenkins_password: Option<String>,
            #[cfg(feature = "jenkins")]
            jenkins_retry_trigger: Option<String>,
            merge_trigger: String,
            check_description: bool,
            check_comments: bool,
            check_own_prs: bool,
            check_approved_prs: bool,
        }

        let mut config_path =
            dirs::home_dir().ok_or_else(|| anyhow!("Couldn't resolve home directory"))?;
        config_path.push(Path::new(".crabby_merge.toml"));
        let config_path: String = config_path
            .into_os_string()
            .into_string()
            .map_err(|path| anyhow!("Couldn't resolve config_path: {}", path.to_string_lossy()))?;

        let config_builder = config::Config::builder()
            .add_source(File::new(&config_path, FileFormat::Toml))
            .add_source(Environment::with_prefix("CRABBY_MERGE"))
            .set_default("merge_trigger", ":shipit:")?
            .set_default("check_description", true)?
            .set_default("check_comments", false)?
            .set_default("check_own_prs", true)?
            .set_default("check_approved_prs", false)?;

        let config: Options = config_builder
            .build()
            .with_context(|| {
                format!(
                "Please set bitbucket_url and bitbucket_api_token either in {} or as environment \
            variables with the prefix \"CRABBY_MERGE\"",
                config_path
            )
            })
            .and_then(|config| {
                config
                    .try_deserialize()
                    .map_err(|_| anyhow!("failed to load config"))
            })?;
        cfg_if::cfg_if! {
            if #[cfg(feature = "jenkins")] {
                let retry_regex = match &config.jenkins_retry_trigger {
                    Some(trigger) => Some(Regex::new(trigger)?),
                    None => None,
                };
            }
        }
        let merge_regex = RegexBuilder::new(&config.merge_trigger)
            .multi_line(true)
            .build()
            .with_context(|| format!("Bad regex: {}", config.merge_trigger))?;
        Ok(Config {
            bitbucket_url: config.bitbucket_url,
            bitbucket_api_token: config.bitbucket_api_token,
            #[cfg(feature = "jenkins")]
            jenkins_auth: match (config.jenkins_username, config.jenkins_password) {
                (Some(username), Some(password)) => Some(jenkins::Auth::new(username, password)),
                _ => None,
            },
            #[cfg(feature = "jenkins")]
            jenkins_retry_regex: retry_regex,
            check_comments: config.check_comments,
            check_description: config.check_description,
            check_own_prs: config.check_own_prs,
            check_approved_prs: config.check_approved_prs,
            merge_regex,
        })
    }
}
