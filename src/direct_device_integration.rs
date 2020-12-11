// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use reqwest::{header, Client};
use thiserror::Error;
use url::Url;

use crate::poll;

#[derive(Debug)]
pub struct DirectDeviceIntegration {
    base_url: Url,
    client: Client,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Could not parse url")]
    ParseUrlError(#[from] url::ParseError),
    #[error("Invalid token format")]
    InvalidToken(#[from] header::InvalidHeaderValue),
    #[error("Failed to process request")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Failed to parse polling sleep")]
    InvalidSleep,
    #[error("No config data has been requested from server")]
    NoConfigData,
}

impl DirectDeviceIntegration {
    pub fn new(
        url: &str,
        tenant: &str,
        controller_id: &str,
        key_token: &str,
    ) -> Result<Self, Error> {
        let host: Url = url.parse()?;
        let path = format!("{}/controller/v1/{}", tenant, controller_id);
        let base_url = host.join(&path)?;

        let mut headers = header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("TargetToken {}", key_token).try_into()?,
        );

        let client = Client::builder().default_headers(headers).build()?;
        Ok(Self { base_url, client })
    }

    pub async fn poll(&self) -> Result<poll::Reply, Error> {
        let reply = self.client.get(self.base_url.clone()).send().await?;
        reply.error_for_status_ref()?;

        let reply = reply.json::<poll::ReplyInternal>().await?;
        Ok(poll::Reply::new(reply, self.client.clone()))
    }
}
