#![cfg(feature = "jenkins")]

use crate::History;
use log::*;
use time::Duration;

fn backoff_time(n_retries: u32) -> Duration {
    if n_retries == 0 {
        Duration::ZERO
    } else {
        Duration::minutes(5)
    }
}

pub fn should_retry_now(hash: &str, max_retries: u32) -> bool {
    match History::load(hash) {
        Err(_) => {
            History::delete(hash).ok();
            false
        }
        Ok(None) => {
            if let Err(e) = History::save(hash, 0) {
                error!("Error saving Jenkins history file for {}: {}", hash, e);
            }
            max_retries > 0 && backoff_time(0) == Duration::ZERO
        }
        Ok(Some(history)) => {
            if history.n_retries() < max_retries
                && history.age() >= backoff_time(history.n_retries())
            {
                if let Err(e) = History::save(hash, history.n_retries() + 1) {
                    error!("Error saving Jenkins history file for {}: {}", hash, e);
                }
                true
            } else {
                false
            }
        }
    }
}
