// Copyright 2025, Liebherr Digital Development Center GmbH.
// SPDX-License-Identifier: MIT OR Apache-2.0

use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

use crate::ddi::client::Error;
use crate::ddi::deployment_base::{Chunk, Deployment};

#[derive(Debug)]
#[allow(dead_code)]
/// A pending confirmation whose details have not been retrieved yet.
///
/// Call [`ConfirmationRequest::fetch()`] to retrieve the details from server.
pub struct ConfirmationRequest {
    client: Client,
    url: String,
    details: Vec<String>,
}

impl ConfirmationRequest {
    pub(crate) fn new(client: Client, url: String) -> Self {
        Self {
            client,
            url,
            details: vec![],
        }
    }

    /// confirm the confirmation request. The server should then proceed with the update and make a deploymentBase available.
    pub async fn confirm(self) -> Result<(), Error> {
        let confirmation = Confirmation::new(ConfirmationResponse::Confirmed, 1);

        // get feedback url
        let mut url: Url = self.url.parse()?;
        {
            let mut paths = url
                .path_segments_mut()
                .map_err(|_| url::ParseError::SetHostOnCannotBeABaseUrl)?;
            paths.push("feedback");
        }
        url.set_query(None);

        let reply = self
            .client
            .post(url.to_string())
            .json(&confirmation)
            .send()
            .await?;
        reply.error_for_status_ref()?;
        Ok(())
    }

    /// decline the confirmation request. This will not change the status on the server and the same confirmation request will be received on the next poll.
    pub async fn decline(self) -> Result<(), Error> {
        let confirmation = Confirmation::new(ConfirmationResponse::Denied, -1);

        // get feedback url
        let mut url: Url = self.url.parse()?;
        {
            let mut paths = url
                .path_segments_mut()
                .map_err(|_| url::ParseError::SetHostOnCannotBeABaseUrl)?;
            paths.push("feedback");
        }
        url.set_query(None);

        let reply = self
            .client
            .post(url.to_string())
            .json(&confirmation)
            .send()
            .await?;
        reply.error_for_status_ref()?;
        Ok(())
    }

    /// Fetch the details of the update to be confirmed
    pub async fn update_info(&self) -> Result<ConfirmationInfo, Error> {
        let reply = self.client.get(&self.url).send().await?;
        reply.error_for_status_ref()?;

        let reply: Reply = reply.json().await?;
        Ok(ConfirmationInfo {
            reply,
            client: self.client.clone(),
        })
    }

    /// The metadata of all chunks of the update.
    pub async fn metadata(&self) -> Result<Vec<(String, String)>, Error> {
        let client = self.client.clone();

        // get update information from the server
        let update = self.update_info().await?.reply;

        // get all chunks of the update
        let chunks: Vec<Chunk> = update
            .confirmation
            .chunks
            .iter()
            .map(move |c| Chunk::new(c, client.clone()))
            .collect();

        // collect all metadata of each chunk
        let metadata = chunks
            .iter()
            .flat_map(|c| c.metadata().collect::<Vec<(&str, &str)>>())
            .map(|(k, v): (&str, &str)| (k.to_string(), v.to_string()))
            .collect();

        Ok(metadata)
    }
}

/// The downloaded details of a confirmation request.
#[derive(Debug)]
pub struct ConfirmationInfo {
    client: Client,
    reply: Reply,
}

impl ConfirmationInfo {
    /// Get all metadata of all chunks of the update.
    pub fn metadata(&self) -> Vec<(String, String)> {
        // get all chunks of the update
        let chunks: Vec<Chunk> = self
            .reply
            .confirmation
            .chunks
            .iter()
            .map(move |c| Chunk::new(c, self.client.clone()))
            .collect();

        // collect all metadata of each chunk
        let metadata = chunks
            .iter()
            .flat_map(|c| c.metadata().collect::<Vec<(&str, &str)>>())
            .map(|(k, v): (&str, &str)| (k.to_string(), v.to_string()))
            .collect();

        metadata
    }

    /// Get the action ID of the update to be confirmed.
    pub fn action_id(&self) -> &str {
        &self.reply.id
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Reply {
    id: String,
    confirmation: Deployment,
}

/// The response to a confirmation request.
#[derive(Debug, Deserialize, Serialize, Copy, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConfirmationResponse {
    /// Proceed with the download
    Confirmed,
    /// Decline the confirmation and do not update
    Denied,
}

#[derive(Debug, Serialize)]
pub struct Confirmation {
    confirmation: ConfirmationResponse,
    code: i32,
    details: Vec<String>,
}

impl Confirmation {
    pub fn new(confirmation: ConfirmationResponse, code: i32) -> Self {
        Self {
            confirmation,
            code,
            details: vec![],
        }
    }
}
