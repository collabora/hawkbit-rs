// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::ddi::client::Error;
use crate::ddi::feedback::Feedback;

#[derive(Debug, Deserialize)]
pub struct Link {
    href: String,
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.href)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
/// Sent by the target to the server informing it about the execution state of a pending request,
/// see the [DDI API reference](https://www.eclipse.org/hawkbit/apis/ddi_api/) for details.
pub enum Execution {
    /// Target completes the action either with `Finished::Success` or `Finished::Failure` as result.
    Closed,
    /// This can be used by the target to inform that it is working on the action.
    Proceeding,
    /// This is send by the target as confirmation of a cancellation request by the update server.
    Canceled,
    /// This can be used by the target to inform that it scheduled on the action.
    Scheduled,
    /// This is send by the target in case an update of a cancellation is rejected, i.e. cannot be fulfilled at this point in time.
    Rejected,
    /// This can be used by the target to inform that it continued to work on the action.
    Resumed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
/// Status of a pending operation
pub enum Finished {
    /// Operation suceeded
    Success,
    /// Operation failed
    Failure,
    /// Operation is still in-progress
    None,
}

pub(crate) async fn send_feedback_internal<T: Serialize>(
    client: &Client,
    url: &str,
    id: &str,
    execution: Execution,
    finished: Finished,
    progress: Option<T>,
    details: Vec<&str>,
) -> Result<(), Error> {
    let mut url: Url = url.parse()?;
    {
        match url.path_segments_mut() {
            Err(_) => {
                return Err(Error::ParseUrlError(
                    url::ParseError::SetHostOnCannotBeABaseUrl,
                ))
            }
            Ok(mut paths) => {
                paths.push("feedback");
            }
        }
    }
    url.set_query(None);

    let details = details.iter().map(|m| m.to_string()).collect();
    let feedback = Feedback::new(id, execution, finished, progress, details);

    let reply = client.post(&url.to_string()).json(&feedback).send().await?;
    reply.error_for_status()?;

    Ok(())
}
