// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use httpmock::{Method::GET, MockRef, MockServer};
use serde_json::json;

pub struct ServerBuilder {
    tenant: String,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self {
            tenant: "DEFAULT".into(),
        }
    }
}

impl ServerBuilder {
    pub fn tenant(self, tenant: &str) -> Self {
        let mut builder = self;
        builder.tenant = tenant.to_string();
        builder
    }

    pub fn build(self) -> Server {
        Server {
            server: MockServer::start(),
            tenant: self.tenant,
        }
    }
}

pub struct Server {
    pub tenant: String,
    server: MockServer,
}

impl Server {
    pub fn base_url(&self) -> String {
        self.server.base_url()
    }

    pub fn add_target(&self, name: &str) -> Target {
        let key = format!("Key{}", name);

        let poll = self.server.mock(|when, then| {
            when.method(GET)
                .path(format!("/{}/controller/v1/{}", self.tenant, name))
                .header("Authorization", &format!("TargetToken {}", key));

            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "config": {
                        "polling": {
                            "sleep": "00:01:00"
                        }
                    }
                }));
        });

        Target {
            name: name.to_string(),
            key,
            poll,
        }
    }
}

pub struct Target<'a> {
    pub name: String,
    pub key: String,
    poll: MockRef<'a>,
}

impl<'a> Target<'a> {
    pub fn poll_hits(&self) -> usize {
        self.poll.hits()
    }
}
