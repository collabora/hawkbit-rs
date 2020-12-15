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
pub enum Execution {
    Closed,
    Proceeding,
    Canceled,
    Scheduled,
    Rejected,
    Resumed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Finished {
    Success,
    Failure,
    None,
}
