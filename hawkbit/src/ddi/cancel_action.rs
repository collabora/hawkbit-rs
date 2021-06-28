// Copyright 2021, Collabora Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

// Cancelled operation

use reqwest::Client;
use serde::Deserialize;

use crate::ddi::client::Error;
use crate::ddi::common::{send_feedback_internal, Execution, Finished};

/// A request from the server to cancel an update.
///
/// Call [`CancelAction::id()`] to retrieve the ID of the action to cancel.
///
/// Cancel actions need to be closed by sending feedback to the server using
/// [`CancelAction::send_feedback`] with either
/// [`Finished::Success`] or [`Finished::Failure`].
#[derive(Debug)]
pub struct CancelAction {
    client: Client,
    url: String,
}

impl CancelAction {
    pub(crate) fn new(client: Client, url: String) -> Self {
        Self { client, url }
    }

    /// Retrieve the id of the action to cancel.
    pub async fn id(&self) -> Result<String, Error> {
        let reply = self.client.get(&self.url).send().await?;
        reply.error_for_status_ref()?;

        let reply = reply.json::<CancelReply>().await?;
        Ok(reply.cancel_action.stop_id)
    }

    /// Send feedback to server about this cancel action.
    ///
    /// # Arguments
    /// * `execution`: status of the action execution.
    /// * `finished`: defined status of the result. The action will be kept open on the server until the controller on the device reports either [`Finished::Success`] or [`Finished::Failure`].
    /// * `details`: list of details message information.
    pub async fn send_feedback(
        &self,
        execution: Execution,
        finished: Finished,
        details: Vec<&str>,
    ) -> Result<(), Error> {
        let id = self.id().await?;

        send_feedback_internal::<bool>(
            &self.client,
            &self.url,
            &id,
            execution,
            finished,
            None,
            details,
        )
        .await
    }
}

#[derive(Debug, Deserialize)]
struct CancelReply {
    id: String,
    #[serde(rename = "cancelAction")]
    cancel_action: CancelActionReply,
}

#[derive(Debug, Deserialize)]
struct CancelActionReply {
    #[serde(rename = "stopId")]
    stop_id: String,
}
