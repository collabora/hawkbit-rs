// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

// Structures used to send config data

use reqwest::Client;
use serde::Serialize;

use crate::direct_device_integration::Error;

#[derive(Debug)]
pub struct Request {
    client: Client,
    url: String,
}

impl Request {
    pub(crate) fn new(client: Client, url: String) -> Self {
        Self { client, url }
    }

    pub async fn upload<T: Serialize>(
        &self,
        execution: Execution,
        finished: Finished,
        mode: Option<Mode>,
        data: T,
    ) -> Result<(), Error> {
        let data = ConfigData::new(execution, finished, mode, data);
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
    // TODO: id?
    // TODO: time
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
    ) -> Self {
        Self {
            data,
            status: Status {
                execution,
                result: ResultT { finished },
                // TODO: add API to pass details?
                details: vec![],
            },
            mode,
        }
    }
}
