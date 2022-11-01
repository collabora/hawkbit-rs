// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

// Structures when querying deployment

use std::path::{Path, PathBuf};

use bytes::Bytes;
use futures::{prelude::*, TryStreamExt};
use reqwest::{Client, Response};
use serde::de::{Deserializer, Error as _, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Serialize};

use tokio::{
    fs::{DirBuilder, File},
    io::AsyncWriteExt,
};

use crate::ddi::client::Error;
use crate::ddi::common::{send_feedback_internal, Execution, Finished, Link};

#[derive(Debug)]
/// A pending update whose details have not been retrieved yet.
///
/// Call [`UpdatePreFetch::fetch()`] to retrieve the details from server.
pub struct UpdatePreFetch {
    client: Client,
    url: String,
}

impl UpdatePreFetch {
    pub(crate) fn new(client: Client, url: String) -> Self {
        Self { client, url }
    }

    /// Retrieve details about the update.
    pub async fn fetch(self) -> Result<Update, Error> {
        let reply = self.client.get(&self.url).send().await?;
        reply.error_for_status_ref()?;

        let reply = reply.json::<Reply>().await?;
        Ok(Update::new(self.client, reply, self.url))
    }
}

#[derive(Debug, Deserialize)]
struct Reply {
    id: String,
    deployment: Deployment,
    #[serde(rename = "actionHistory")]
    #[allow(dead_code)]
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

/// How the download or update should be processed by the target.
#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    /// Do not process yet
    Skip,
    /// Server asks to process
    Attempt,
    /// Server requests immediate processing
    Forced,
}

/// Separation of download and installation by defining a maintenance window for the installation.
#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MaintenanceWindow {
    /// Maintenance window is available
    Available,
    /// Maintenance window is unavailable
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
    #[cfg(feature = "hash-sha1")]
    sha1: String,
    #[cfg(feature = "hash-md5")]
    md5: String,
    #[cfg(feature = "hash-sha256")]
    sha256: String,
}

impl<'de> Deserialize<'de> for Links {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Links;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut download: Option<Link> = None;
                let mut md5sum: Option<Link> = None;
                let mut download_http: Option<Link> = None;
                let mut md5sum_http: Option<Link> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "download" => {
                            download = match download {
                                Some(_) => return Err(A::Error::duplicate_field("download")),
                                None => Some(map.next_value()?),
                            };
                        }
                        "md5sum" => {
                            md5sum = match md5sum {
                                Some(_) => return Err(A::Error::duplicate_field("md5sum")),
                                None => Some(map.next_value()?),
                            };
                        }
                        "download-http" => {
                            download_http = match download_http {
                                Some(_) => return Err(A::Error::duplicate_field("download-http")),
                                None => Some(map.next_value()?),
                            };
                        }
                        "md5sum-http" => {
                            md5sum_http = match md5sum_http {
                                Some(_) => return Err(A::Error::duplicate_field("md5sum-http")),
                                None => Some(map.next_value()?),
                            };
                        }
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                let https = download.map(|content| Download { content, md5sum });
                let http = download_http.map(|content| Download {
                    content,
                    md5sum: md5sum_http,
                });

                if http.is_none() && https.is_none() {
                    Err(A::Error::missing_field("download or download-http"))
                } else {
                    Ok(Links { http, https })
                }
            }
        }

        let visitor = V;

        deserializer.deserialize_map(visitor)
    }
}

#[derive(Debug)]
struct Download {
    content: Link,
    #[allow(dead_code)]
    md5sum: Option<Link>,
}

/// Download links a single artifact, at least one of http or https will be
/// Some
#[derive(Debug)]
struct Links {
    http: Option<Download>,
    https: Option<Download>,
}

#[derive(Debug, Deserialize)]
struct ActionHistory {
    #[allow(dead_code)]
    status: String,
    #[serde(default)]
    #[allow(dead_code)]
    messages: Vec<String>,
}

/// A pending update to deploy.
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

    /// Handling for the download part of the provisioning process.
    pub fn download_type(&self) -> Type {
        self.info.deployment.download
    }

    /// Handling for the update part of the provisioning process.
    pub fn update_type(&self) -> Type {
        self.info.deployment.update
    }

    /// If set, the update is part of a maintenance window.
    pub fn maintenance_window(&self) -> Option<MaintenanceWindow> {
        self.info.deployment.maintenance_window
    }

    /// An iterator on all the software chunks of the update.
    pub fn chunks(&self) -> impl Iterator<Item = Chunk> {
        let client = self.client.clone();

        self.info
            .deployment
            .chunks
            .iter()
            .map(move |c| Chunk::new(c, client.clone()))
    }

    /// Download all software chunks to the directory defined in `dir`.
    pub async fn download(&self, dir: &Path) -> Result<Vec<DownloadedArtifact>, Error> {
        let mut result = Vec::new();
        for c in self.chunks() {
            let downloaded = c.download(dir).await?;
            result.extend(downloaded);
        }

        Ok(result)
    }

    /// Send feedback to server about this update, with custom progress information.
    ///
    /// # Arguments
    /// * `execution`: status of the action execution.
    /// * `finished`: defined status of the result. The action will be kept open on the server until the controller on the device reports either [`Finished::Success`] or [`Finished::Failure`].
    /// * `progress`: progress assumption of the device.
    /// * `details`: list of details message information.
    pub async fn send_feedback_with_progress<T: Serialize>(
        &self,
        execution: Execution,
        finished: Finished,
        progress: T,
        details: Vec<&str>,
    ) -> Result<(), Error> {
        send_feedback_internal(
            &self.client,
            &self.url,
            &self.info.id,
            execution,
            finished,
            Some(progress),
            details,
        )
        .await
    }

    /// Send feedback to server about this update.
    ///
    /// Same as [`Update::send_feedback_with_progress`] but without passing custom progress information about the update.
    pub async fn send_feedback(
        &self,
        execution: Execution,
        finished: Finished,
        details: Vec<&str>,
    ) -> Result<(), Error> {
        send_feedback_internal::<bool>(
            &self.client,
            &self.url,
            &self.info.id,
            execution,
            finished,
            None,
            details,
        )
        .await
    }
}

/// Software chunk of an update.
#[derive(Debug)]
pub struct Chunk<'a> {
    chunk: &'a ChunkInternal,
    client: Client,
}

impl<'a> Chunk<'a> {
    fn new(chunk: &'a ChunkInternal, client: Client) -> Self {
        Self { chunk, client }
    }

    /// Type of the chunk.
    pub fn part(&self) -> &str {
        &self.chunk.part
    }

    /// Name of the chunk.
    pub fn name(&self) -> &str {
        &self.chunk.name
    }

    /// Software version of the chunk.
    pub fn version(&self) -> &str {
        &self.chunk.version
    }

    /// An iterator on all the artifacts of the chunk.
    pub fn artifacts(&self) -> impl Iterator<Item = Artifact> {
        let client = self.client.clone();

        self.chunk
            .artifacts
            .iter()
            .map(move |a| Artifact::new(a, client.clone()))
    }

    /// An iterator on all the metadata of the chunk.
    pub fn metadata(&self) -> impl Iterator<Item = (&str, &str)> {
        self.chunk
            .metadata
            .iter()
            .map(|a| (a.key.as_str(), a.value.as_str()))
    }

    /// Download all artifacts of the chunk to the directory defined in `dir`.
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

/// A single file part of a [`Chunk`] to download.
#[derive(Debug)]
pub struct Artifact<'a> {
    artifact: &'a ArtifactInternal,
    client: Client,
}

impl<'a> Artifact<'a> {
    fn new(artifact: &'a ArtifactInternal, client: Client) -> Self {
        Self { artifact, client }
    }

    /// The name of the file.
    pub fn filename(&self) -> &str {
        &self.artifact.filename
    }

    /// The size of the file.
    pub fn size(&self) -> u32 {
        self.artifact.size
    }

    async fn download_response(&'a self) -> Result<Response, Error> {
        let download = self
            .artifact
            .links
            .https
            .as_ref()
            .or(self.artifact.links.http.as_ref())
            .expect("Missing content link in for artifact");

        let resp = self
            .client
            .get(&download.content.to_string())
            .send()
            .await?;

        resp.error_for_status_ref()?;
        Ok(resp)
    }

    /// Download the artifact file to the directory defined in `dir`.
    pub async fn download(&'a self, dir: &Path) -> Result<DownloadedArtifact, Error> {
        let mut resp = self.download_response().await?;

        if !dir.exists() {
            DirBuilder::new().recursive(true).create(dir).await?;
        }

        let mut file_name = dir.to_path_buf();
        file_name.push(self.filename());
        let mut dest = File::create(&file_name).await?;

        while let Some(chunk) = resp.chunk().await? {
            dest.write_all(&chunk).await?;
        }

        Ok(DownloadedArtifact::new(
            file_name,
            self.artifact.hashes.clone(),
        ))
    }

    /// Provide a `Stream` of `Bytes` to download the artifact.
    ///
    /// This can be used as an alternative to [`Artifact::download`],
    /// for example, to extract an archive while it's being downloaded,
    /// saving the need to store the archive file on disk.
    pub async fn download_stream(
        &'a self,
    ) -> Result<impl Stream<Item = Result<Bytes, Error>>, Error> {
        let resp = self.download_response().await?;

        Ok(resp.bytes_stream().map_err(|e| e.into()))
    }

    /// Provide a `Stream` of `Bytes` to download the artifact while checking md5 checksum.
    ///
    /// The stream will yield the same data as [`Artifact::download_stream`] but will raise
    /// an error if the md5sum of the downloaded data does not match the one provided by the server.
    #[cfg(feature = "hash-md5")]
    pub async fn download_stream_with_md5_check(
        &'a self,
    ) -> Result<impl Stream<Item = Result<Bytes, Error>>, Error> {
        let stream = self.download_stream().await?;
        let hasher = DownloadHasher::new_md5(self.artifact.hashes.md5.clone());

        let stream = DownloadStreamHash {
            stream: Box::new(stream),
            hasher,
        };

        Ok(stream)
    }

    /// Provide a `Stream` of `Bytes` to download the artifact while checking sha1 checksum.
    ///
    /// The stream will yield the same data as [`Artifact::download_stream`] but will raise
    /// an error if the sha1sum of the downloaded data does not match the one provided by the server.
    #[cfg(feature = "hash-sha1")]
    pub async fn download_stream_with_sha1_check(
        &'a self,
    ) -> Result<impl Stream<Item = Result<Bytes, Error>>, Error> {
        let stream = self.download_stream().await?;
        let hasher = DownloadHasher::new_sha1(self.artifact.hashes.sha1.clone());

        let stream = DownloadStreamHash {
            stream: Box::new(stream),
            hasher,
        };

        Ok(stream)
    }

    /// Provide a `Stream` of `Bytes` to download the artifact while checking sha256 checksum.
    ///
    /// The stream will yield the same data as [`Artifact::download_stream`] but will raise
    /// an error if the sha256sum of the downloaded data does not match the one provided by the server.
    #[cfg(feature = "hash-sha256")]
    pub async fn download_stream_with_sha256_check(
        &'a self,
    ) -> Result<impl Stream<Item = Result<Bytes, Error>>, Error> {
        let stream = self.download_stream().await?;
        let hasher = DownloadHasher::new_sha256(self.artifact.hashes.sha256.clone());

        let stream = DownloadStreamHash {
            stream: Box::new(stream),
            hasher,
        };

        Ok(stream)
    }
}

/// A downloaded file part of a [`Chunk`].
#[derive(Debug)]
pub struct DownloadedArtifact {
    file: PathBuf,
    #[allow(dead_code)]
    hashes: Hashes,
}

cfg_if::cfg_if! {
    if #[cfg(feature = "hash-digest")] {
        use std::{
            pin::Pin,
            task::Poll,
        };
        use digest::Digest;
        use digest::OutputSizeUser;

        const HASH_BUFFER_SIZE: usize = 4096;

        /// Enum representing the different type of supported checksums
        #[derive(Debug, strum::Display, Clone)]
        pub enum ChecksumType {
            /// md5
            #[cfg(feature = "hash-md5")]
            Md5,
            /// sha1
            #[cfg(feature = "hash-sha1")]
            Sha1,
            /// sha256
            #[cfg(feature = "hash-sha256")]
            Sha256,
        }

        // quite complex trait bounds because of requirements so LowerHex is implemented on the output
        #[derive(Clone)]
        struct DownloadHasher<T>
        where
            T: Digest,
            <T as OutputSizeUser>::OutputSize: core::ops::Add,
            <<T as OutputSizeUser>::OutputSize as core::ops::Add>::Output: generic_array::ArrayLength<u8>,

        {
            hasher: T,
            expected: String,
            error: ChecksumType,
        }

        impl<T> DownloadHasher<T>
        where
            T: Digest,
            <T as OutputSizeUser>::OutputSize: core::ops::Add,
            <<T as OutputSizeUser>::OutputSize as core::ops::Add>::Output: generic_array::ArrayLength<u8>,
        {
            fn update(&mut self, data: impl AsRef<[u8]>) {
                self.hasher.update(data);
            }

            fn finalize(self) -> Result<(), Error> {
                let digest = self.hasher.finalize();

                if format!("{:x}", digest) == self.expected {
                    Ok(())
                } else {
                    Err(Error::ChecksumError(self.error))
                }
            }
        }

        #[cfg(feature = "hash-md5")]
        impl DownloadHasher<md5::Md5> {
            fn new_md5(expected: String) -> Self {
                Self {
                    hasher: md5::Md5::new(),
                    expected,
                    error: ChecksumType::Md5,
                }
            }
        }

        #[cfg(feature = "hash-sha1")]
        impl DownloadHasher<sha1::Sha1> {
            fn new_sha1(expected: String) -> Self {
                Self {
                    hasher: sha1::Sha1::new(),
                    expected,
                    error: ChecksumType::Sha1,
                }
            }
        }

        #[cfg(feature = "hash-sha256")]
        impl DownloadHasher<sha2::Sha256> {
            fn new_sha256(expected: String) -> Self {
                Self {
                    hasher: sha2::Sha256::new(),
                    expected,
                    error: ChecksumType::Sha256,
                }
            }
        }

        struct DownloadStreamHash<T>
        where
            T: Digest,
            <T as OutputSizeUser>::OutputSize: core::ops::Add,
            <<T as OutputSizeUser>::OutputSize as core::ops::Add>::Output: generic_array::ArrayLength<u8>,
        {
            stream: Box<dyn Stream<Item = Result<Bytes, Error>> + Unpin + Send + Sync>,
            hasher: DownloadHasher<T>,
        }

        impl<T> Stream for DownloadStreamHash<T>
        where
            T: Digest,
            T: Unpin,
            T: Clone,
            <T as OutputSizeUser>::OutputSize: core::ops::Add,
            <<T as OutputSizeUser>::OutputSize as core::ops::Add>::Output: generic_array::ArrayLength<u8>,
        {
            type Item = Result<Bytes, Error>;

            fn poll_next(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                let me = Pin::into_inner(self);

                match Pin::new(&mut me.stream).poll_next(cx) {
                    Poll::Ready(Some(Ok(data))) => {
                        // feed data to the hasher and then pass them back to the stream
                        me.hasher.update(&data);
                        Poll::Ready(Some(Ok(data)))
                    }
                    Poll::Ready(None) => {
                        // download is done, check the hash
                        match me.hasher.clone().finalize() {
                            Ok(_) => Poll::Ready(None),
                            Err(e) => Poll::Ready(Some(Err(e))),
                        }
                    }
                    // passthrough on errors and pendings
                    Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }
}

impl DownloadedArtifact {
    fn new(file: PathBuf, hashes: Hashes) -> Self {
        Self { file, hashes }
    }

    /// Path of the downloaded file.
    pub fn file(&self) -> &PathBuf {
        &self.file
    }

    #[cfg(feature = "hash-digest")]
    async fn hash<T>(&self, mut hasher: DownloadHasher<T>) -> Result<(), Error>
    where
        T: Digest,
        <T as OutputSizeUser>::OutputSize: core::ops::Add,
        <<T as OutputSizeUser>::OutputSize as core::ops::Add>::Output:
            generic_array::ArrayLength<u8>,
    {
        use tokio::io::AsyncReadExt;

        let mut file = File::open(&self.file).await?;
        let mut buffer = [0; HASH_BUFFER_SIZE];

        loop {
            let n = file.read(&mut buffer[..]).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        hasher.finalize()
    }

    /// Check if the md5sum of the downloaded file matches the one provided by the server.
    #[cfg(feature = "hash-md5")]
    pub async fn check_md5(&self) -> Result<(), Error> {
        let hasher = DownloadHasher::new_md5(self.hashes.md5.clone());
        self.hash(hasher).await
    }

    /// Check if the sha1sum of the downloaded file matches the one provided by the server.
    #[cfg(feature = "hash-sha1")]
    pub async fn check_sha1(&self) -> Result<(), Error> {
        let hasher = DownloadHasher::new_sha1(self.hashes.sha1.clone());
        self.hash(hasher).await
    }

    /// Check if the sha256sum of the downloaded file matches the one provided by the server.
    #[cfg(feature = "hash-sha256")]
    pub async fn check_sha256(&self) -> Result<(), Error> {
        let hasher = DownloadHasher::new_sha256(self.hashes.sha256.clone());
        self.hash(hasher).await
    }
}
