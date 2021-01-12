// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::fmt;

use serde::{Deserialize, Serialize};

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
