// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

// Structures when querying deployment

use reqwest::Client;
use serde::Deserialize;

use crate::direct_device_integration::Error;
use crate::poll::Link;

#[derive(Debug)]
pub struct UpdatePreFetch {
    client: Client,
    url: String,
}

impl UpdatePreFetch {
    pub(crate) fn new(client: Client, url: String) -> Self {
        Self { client, url }
    }

    pub async fn fetch(self) -> Result<Update, Error> {
        let reply = self.client.get(&self.url).send().await?;
        reply.error_for_status_ref()?;

        let reply = reply.json::<Reply>().await?;
        Ok(Update::new(self.client, reply))
    }
}

#[derive(Debug, Deserialize)]
pub struct Reply {
    id: String,
    deployment: Deployment,
    #[serde(rename = "actionHistory")]
    action_history: Option<ActionHistory>,
}

#[derive(Debug, Deserialize)]
struct Deployment {
    download: Type,
    update: Type,
    #[serde(rename = "maintenanceWindow")]
    maintenance_window: Option<MaintenanceWindow>,
    chunks: Vec<Chunk>,
}

#[derive(Debug, Deserialize, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Skip,
    Attempt,
    Forced,
}

#[derive(Debug, Deserialize, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MaintenanceWindow {
    Available,
    Unavailable,
}

#[derive(Debug, Deserialize)]
struct Chunk {
    #[serde(default)]
    metadata: Vec<Metadata>,
    part: String,
    name: String,
    version: String,
    artifacts: Vec<Artifact>,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct Artifact {
    filename: String,
    hashes: Hashes,
    size: u32,
    #[serde(rename = "_links")]
    links: Links,
}

#[derive(Debug, Deserialize)]
struct Hashes {
    sha1: String,
    md5: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct Links {
    #[serde(rename = "download-http")]
    download_http: Link,
    #[serde(rename = "md5sum-http")]
    md5sum_http: Link,
}

#[derive(Debug, Deserialize)]
struct ActionHistory {
    status: String,
    #[serde(default)]
    messages: Vec<String>,
}

#[derive(Debug)]
pub struct Update {
    client: Client,
    info: Reply,
}

impl Update {
    fn new(client: Client, info: Reply) -> Self {
        Self { client, info }
    }

    pub fn download_type(&self) -> Type {
        self.info.deployment.download
    }

    pub fn update_type(&self) -> Type {
        self.info.deployment.update
    }

    pub fn maintenance_window(&self) -> Option<MaintenanceWindow> {
        self.info.deployment.maintenance_window
    }
}
