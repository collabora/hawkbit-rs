// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

// Structures used to poll the status

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::ddi::cancel_action::CancelAction;
use crate::ddi::client::Error;
use crate::ddi::common::Link;
use crate::ddi::config_data::ConfigRequest;
use crate::ddi::deployment_base::UpdatePreFetch;

#[derive(Debug, Deserialize)]
pub(crate) struct ReplyInternal {
    config: Config,
    #[serde(rename = "_links")]
    links: Option<Links>,
}
#[derive(Debug, Deserialize)]
pub struct Config {
    polling: Polling,
}
#[derive(Debug, Deserialize)]
pub struct Polling {
    sleep: String,
}
#[derive(Debug, Deserialize)]
pub struct Links {
    #[serde(rename = "configData")]
    config_data: Option<Link>,
    #[serde(rename = "deploymentBase")]
    deployment_base: Option<Link>,
    #[serde(rename = "cancelAction")]
    cancel_action: Option<Link>,
}

/// Polling reply from the server
#[derive(Debug)]
pub struct Reply {
    reply: ReplyInternal,
    client: Client,
}

impl Reply {
    pub(crate) fn new(reply: ReplyInternal, client: Client) -> Self {
        Self { reply, client }
    }

    /// Suggested sleeping time between two polling requests to the server.
    pub fn polling_sleep(&self) -> Result<Duration, Error> {
        self.reply.config.polling.as_duration()
    }

    /// Returns pending configuration data request from the server, if any.
    pub fn config_data_request(&self) -> Option<ConfigRequest> {
        match &self.reply.links {
            Some(links) => links
                .config_data
                .as_ref()
                .map(|l| ConfigRequest::new(self.client.clone(), l.to_string())),
            None => None,
        }
    }

    /// Returns pending update to deploy, if any.
    pub fn update(&self) -> Option<UpdatePreFetch> {
        match &self.reply.links {
            Some(links) => links
                .deployment_base
                .as_ref()
                .map(|l| UpdatePreFetch::new(self.client.clone(), l.to_string())),
            None => None,
        }
    }

    /// Returns pending cancel action, if any.
    pub fn cancel_action(&self) -> Option<CancelAction> {
        match &self.reply.links {
            Some(links) => links
                .cancel_action
                .as_ref()
                .map(|l| CancelAction::new(self.client.clone(), l.to_string())),
            None => None,
        }
    }
}

impl Polling {
    fn as_duration(&self) -> Result<Duration, Error> {
        let times: Vec<Result<u64, _>> = self.sleep.split(':').map(|s| s.parse()).collect();
        if times.len() != 3 {
            return Err(Error::InvalidSleep);
        }

        match times[..] {
            [Ok(h), Ok(m), Ok(s)] => Ok(Duration::new(h * 60 * 60 + m * 60 + s, 0)),
            _ => Ok(Duration::new(0, 0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_duration() {
        let polling = Polling {
            sleep: "00:00:05".to_string(),
        };
        assert_eq!(polling.as_duration().unwrap(), Duration::new(5, 0));

        let polling = Polling {
            sleep: "00:05:05".to_string(),
        };
        assert_eq!(polling.as_duration().unwrap(), Duration::new(305, 0));

        let polling = Polling {
            sleep: "01:05:05".to_string(),
        };
        assert_eq!(polling.as_duration().unwrap(), Duration::new(3905, 0));

        let polling = Polling {
            sleep: "05:05".to_string(),
        };
        assert!(polling.as_duration().is_err());

        let polling = Polling {
            sleep: "invalid".to_string(),
        };
        assert!(polling.as_duration().is_err());
    }
}
