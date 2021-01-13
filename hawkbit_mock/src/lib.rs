// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

#![warn(missing_docs)]

//! # hawkbit_mock
//!
//! Mock server implementation of [Eclipse hawkBit](https://www.eclipse.org/hawkbit/)
//! using [httpmock](https://crates.io/crates/httpmock).

//! This mock is used to test the `hawkbit` crate but can also be useful to test any `hawkBit` client.
//! So far only the [Direct Device Integration API](https://www.eclipse.org/hawkbit/apis/ddi_api/)
//! is implemented, see the [`ddi`] module.

pub mod ddi;
