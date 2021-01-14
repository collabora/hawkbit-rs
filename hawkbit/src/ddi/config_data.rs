// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

// Structures used to send config data

use reqwest::Client;
use serde::Serialize;

use crate::ddi::{Error, Execution, Finished};

/// A request from the server asking to upload the device configuration.
#[derive(Debug)]
pub struct ConfigRequest {
    client: Client,
    url: String,
}

impl ConfigRequest {
    pub(crate) fn new(client: Client, url: String) -> Self {
        Self { client, url }
    }

    /// Send the requested device configuration to the server.
    ///
    /// The configuration is represented as the `data` argument which
    /// need to be serializable.
    pub async fn upload<T: Serialize>(
        &self,
        execution: Execution,
        finished: Finished,
        mode: Option<Mode>,
        data: T,
        details: Vec<&str>,
    ) -> Result<(), Error> {
        let details = details.iter().map(|m| m.to_string()).collect();
        let data = ConfigData::new(execution, finished, mode, data, details);
        let reply = self.client.put(&self.url).json(&data).send().await?;

        reply.error_for_status()?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ConfigData<T: Serialize> {
    status: Status,
    mode: Option<Mode>,
    data: T,
    // skip 'id' as its semantic is unclear and it's left empty in the doc
}
#[derive(Debug, Serialize)]
struct Status {
    execution: Execution,
    result: ResultT,
    details: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ResultT {
    finished: Finished,
}

/// Update mode that should be applied when updating target
// FIXME: would be good to have better documentation of the fields but the spec does not say much
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Merge
    Merge,
    /// Replace
    Replace,
    /// Remove
    Remove,
}

impl<T: Serialize> ConfigData<T> {
    pub(crate) fn new(
        execution: Execution,
        finished: Finished,
        mode: Option<Mode>,
        data: T,
        details: Vec<String>,
    ) -> Self {
        Self {
            data,
            status: Status {
                execution,
                result: ResultT { finished },
                details,
            },
            mode,
        }
    }
}
