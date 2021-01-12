// Copyright 2020-2021, Collabora Ltd.
// SPDX-License-Identifier: MIT

mod client;
mod common;
mod config_data;
mod deployment_base;
mod feedback;
mod poll;

pub use client::{Client, Error};
pub use common::{Execution, Finished};
pub use config_data::{ConfigRequest, Mode};
pub use deployment_base::{
    Artifact, Chunk, DownloadedArtifact, MaintenanceWindow, Type, Update, UpdatePreFetch,
};
pub use poll::Reply;
