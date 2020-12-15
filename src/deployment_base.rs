// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

// Structures when querying deployment

use std::path::{Path, PathBuf};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{DirBuilder, File},
    io::AsyncReadExt,
};
use url::Url;

use crate::common::{Execution, Finished, Link};
use crate::direct_device_integration::Error;
use crate::feedback::Feedback;

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
        Ok(Update::new(self.client, reply, self.url))
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
    chunks: Vec<ChunkInternal>,
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
struct ChunkInternal {
    #[serde(default)]
    metadata: Vec<Metadata>,
    part: String,
    name: String,
    version: String,
    artifacts: Vec<ArtifactInternal>,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct ArtifactInternal {
    filename: String,
    hashes: Hashes,
    size: u32,
    #[serde(rename = "_links")]
    links: Links,
}

#[derive(Debug, Deserialize, Clone)]
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
    url: String,
}

impl Update {
    fn new(client: Client, info: Reply, url: String) -> Self {
        Self { client, info, url }
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

    pub fn chunks(&self) -> impl Iterator<Item = Chunk> {
        let client = self.client.clone();

        self.info
            .deployment
            .chunks
            .iter()
            .map(move |c| Chunk::new(c, client.clone()))
    }

    pub async fn download(&self, dir: &Path) -> Result<Vec<DownloadedArtifact>, Error> {
        let mut result = Vec::new();
        for c in self.chunks() {
            let downloaded = c.download(dir).await?;
            result.extend(downloaded);
        }

        Ok(result)
    }

    pub async fn send_feedback_with_progress<T: Serialize>(
        &self,
        execution: Execution,
        finished: Finished,
        progress: Option<T>,
        details: Vec<&str>,
    ) -> Result<(), Error> {
        let mut url: Url = self.url.parse()?;
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
        let feedback = Feedback::new(&self.info.id, execution, finished, progress, details);

        let reply = self
            .client
            .post(&url.to_string())
            .json(&feedback)
            .send()
            .await?;
        reply.error_for_status()?;

        Ok(())
    }

    pub async fn send_feedback(
        &self,
        execution: Execution,
        finished: Finished,
        details: Vec<&str>,
    ) -> Result<(), Error> {
        self.send_feedback_with_progress::<bool>(execution, finished, None, details)
            .await
    }
}

#[derive(Debug)]
pub struct Chunk<'a> {
    chunk: &'a ChunkInternal,
    client: Client,
}

impl<'a> Chunk<'a> {
    fn new(chunk: &'a ChunkInternal, client: Client) -> Self {
        Self { chunk, client }
    }

    pub fn part(&self) -> &str {
        &self.chunk.part
    }

    pub fn name(&self) -> &str {
        &self.chunk.name
    }

    pub fn version(&self) -> &str {
        &self.chunk.version
    }

    pub fn artifacts(&self) -> impl Iterator<Item = Artifact> {
        let client = self.client.clone();

        self.chunk
            .artifacts
            .iter()
            .map(move |a| Artifact::new(a, client.clone()))
    }

    pub async fn download(&'a self, dir: &Path) -> Result<Vec<DownloadedArtifact>, Error> {
        let mut dir = dir.to_path_buf();
        dir.push(self.name());
        let mut result = Vec::new();

        for a in self.artifacts() {
            let downloaded = a.download(&dir).await?;
            result.push(downloaded);
        }

        Ok(result)
    }
}

#[derive(Debug)]
pub struct Artifact<'a> {
    artifact: &'a ArtifactInternal,
    client: Client,
}

impl<'a> Artifact<'a> {
    fn new(artifact: &'a ArtifactInternal, client: Client) -> Self {
        Self { artifact, client }
    }

    pub fn filename(&self) -> &str {
        &self.artifact.filename
    }

    pub fn size(&self) -> u32 {
        self.artifact.size
    }

    pub async fn download(&'a self, dir: &Path) -> Result<DownloadedArtifact, Error> {
        let resp = self
            .client
            .get(&self.artifact.links.download_http.to_string())
            .send()
            .await?;

        resp.error_for_status_ref()?;

        if !dir.exists() {
            DirBuilder::new().recursive(true).create(dir).await?;
        }

        let mut file_name = dir.to_path_buf();
        file_name.push(self.filename());
        let mut dest = File::create(&file_name).await?;

        let content = resp.bytes().await?;
        tokio::io::copy(&mut content.as_ref(), &mut dest).await?;

        Ok(DownloadedArtifact::new(
            file_name,
            self.artifact.hashes.clone(),
        ))
    }
}

#[derive(Debug)]
pub struct DownloadedArtifact {
    file: PathBuf,
    hashes: Hashes,
}

cfg_if::cfg_if! {
    if #[cfg(feature = "hash-digest")] {
        use digest::Digest;
        use thiserror::Error;

        #[derive(Error, Debug)]
        pub enum ChecksumError {
            #[error("Failed to compute checksum")]
            Io(#[from] std::io::Error),
            #[error("Checksum {0} does not match")]
            Invalid(CheckSumType),
        }

        #[derive(Debug, strum::Display)]
        pub enum CheckSumType {
            #[cfg(feature = "hash-md5")]
            Md5,
            #[cfg(feature = "hash-sha1")]
            Sha1,
            #[cfg(feature = "hash-sha256")]
            Sha256,
        }
    }
}

impl<'a> DownloadedArtifact {
    fn new(file: PathBuf, hashes: Hashes) -> Self {
        Self { file, hashes }
    }

    pub fn file(&self) -> &PathBuf {
        &self.file
    }

    #[cfg(feature = "hash-digest")]
    async fn read_content(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut content = Vec::new();
        let mut file = File::open(&self.file).await?;
        file.read_to_end(&mut content).await?;

        Ok(content)
    }

    #[cfg(feature = "hash-md5")]
    pub async fn check_md5(&self) -> Result<(), ChecksumError> {
        let content = self.read_content().await?;
        let digest = md5::Md5::digest(&content);

        if format!("{:x}", digest) == self.hashes.md5 {
            Ok(())
        } else {
            Err(ChecksumError::Invalid(CheckSumType::Md5))
        }
    }

    #[cfg(feature = "hash-sha1")]
    pub async fn check_sha1(&self) -> Result<(), ChecksumError> {
        let content = self.read_content().await?;
        let digest = sha1::Sha1::digest(&content);

        if format!("{:x}", digest) == self.hashes.sha1 {
            Ok(())
        } else {
            Err(ChecksumError::Invalid(CheckSumType::Sha1))
        }
    }

    #[cfg(feature = "hash-sha256")]
    pub async fn check_sha256(&self) -> Result<(), ChecksumError> {
        let content = self.read_content().await?;
        let digest = sha2::Sha256::digest(&content);

        if format!("{:x}", digest) == self.hashes.sha256 {
            Ok(())
        } else {
            Err(ChecksumError::Invalid(CheckSumType::Sha256))
        }
    }
}
