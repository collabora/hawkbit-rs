// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

// Structures used to send feedback on upgrades

use serde::Serialize;

use crate::ddi::common::{Execution, Finished};

#[derive(Debug, Serialize)]
pub(crate) struct Feedback<T: Serialize> {
    id: String,
    status: Status<T>,
}
#[derive(Debug, Serialize)]
struct Status<T: Serialize> {
    execution: Execution,
    result: ResultT<T>,
    details: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ResultT<T: Serialize> {
    finished: Finished,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress: Option<T>,
}

impl<T: Serialize> Feedback<T> {
    pub(crate) fn new(
        id: &str,
        execution: Execution,
        finished: Finished,
        progress: Option<T>,
        details: Vec<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            status: Status {
                execution,
                details,
                result: ResultT { finished, progress },
            },
        }
    }
}
