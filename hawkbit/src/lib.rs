// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

//! # hawkbit
//!
//! The `hawkbit` crate provides high-level client-side API to interact with
//! [Eclipse hawkBit](https://www.eclipse.org/hawkbit/).
//!
//! So far only the [Direct Device Integration API](https://www.eclipse.org/hawkbit/apis/ddi_api/)
//! is implemented, see the [`ddi`] module.

pub mod ddi;
