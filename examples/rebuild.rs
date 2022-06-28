use anyhow::Result;
use crabby_merge::jenkins;
use std::env;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let url = env::var("JENKINS_JOB_URL").expect("JENKINS_JOB_URL not set");
    let username = env::var("JENKINS_USERNAME").expect("JENKINS_USERNAME not set");
    let password = env::var("JENKINS_PASSWORD").expect("JENKINS_PASSWORD not set");
    let auth = jenkins::Auth::new(username, password);
    let client = reqwest::Client::new();
    let job = jenkins::Job::new(&url, auth)?;
    job.rebuild(&client).await?;
    println!("Rebuilt ✅");
    Ok(())
}
