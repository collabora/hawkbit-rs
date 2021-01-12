// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use thiserror::Error;
use url::Url;

use crate::ddi::poll;

/// [Direct Device Integration](https://www.eclipse.org/hawkbit/apis/ddi_api/) client.
#[derive(Debug)]
pub struct Client {
    base_url: Url,
    client: reqwest::Client,
}

/// DDI errors
#[derive(Error, Debug)]
pub enum Error {
    /// URL error
    #[error("Could not parse url")]
    ParseUrlError(#[from] url::ParseError),
    /// Token error
    #[error("Invalid token format")]
    InvalidToken(#[from] reqwest::header::InvalidHeaderValue),
    /// HTTP error
    #[error("Failed to process request")]
    ReqwestError(#[from] reqwest::Error),
    /// Error parsing sleep field from server
    #[error("Failed to parse polling sleep")]
    InvalidSleep,
    /// IO error
    #[error("Failed to download update")]
    Io(#[from] std::io::Error),
}

impl Client {
    /// Create a new DDI client.
    ///
    /// # Arguments
    /// * `url`: the URL of the hawkBit server, such as `http://my-server.com:8080`
    /// * `tenant`: the server tenant
    /// * `controller_id`: the id of the controller
    /// * `key_token`: the secret authentification token of the controller
    pub fn new(
        url: &str,
        tenant: &str,
        controller_id: &str,
        key_token: &str,
    ) -> Result<Self, Error> {
        let host: Url = url.parse()?;
        let path = format!("{}/controller/v1/{}", tenant, controller_id);
        let base_url = host.join(&path)?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("TargetToken {}", key_token).try_into()?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self { base_url, client })
    }

    /// Poll the server for updates
    pub async fn poll(&self) -> Result<poll::Reply, Error> {
        let reply = self.client.get(self.base_url.clone()).send().await?;
        reply.error_for_status_ref()?;

        let reply = reply.json::<poll::ReplyInternal>().await?;
        Ok(poll::Reply::new(reply, self.client.clone()))
    }
}
