// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

// Structures used to send config data

use reqwest::Client;
use serde::Serialize;

use crate::ddi::{Error, Execution, Finished};

#[derive(Debug)]
pub struct ConfigRequest {
    client: Client,
    url: String,
}

impl ConfigRequest {
    pub(crate) fn new(client: Client, url: String) -> Self {
        Self { client, url }
    }

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Merge,
    Replace,
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
