// Copyright 2020-2021, Collabora Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [Direct Device Integration](https://www.eclipse.org/hawkbit/apis/ddi_api/) API
//!
//! This module provides API for devices to poll their hawkBit server, upload their configuration
//! and download updates.
//!
//! Devices would typically create a [`Client`] using [`Client::new`]
//! and would then regularly call [`Client::poll`] checking for updates.
//!
//! See `examples/polling.rs` demonstrating how to use it.

// FIXME: set link to hawbit/examples/polling.rs once we have the final public repo

mod client;
mod common;
mod config_data;
mod deployment_base;
mod feedback;
mod poll;

pub use client::{Client, Error};
pub use common::{Execution, Finished};
pub use config_data::{ConfigRequest, Mode};
#[cfg(feature = "hash-digest")]
pub use deployment_base::ChecksumType;
pub use deployment_base::{
    Artifact, Chunk, DownloadedArtifact, MaintenanceWindow, Type, Update, UpdatePreFetch,
};
pub use poll::Reply;
