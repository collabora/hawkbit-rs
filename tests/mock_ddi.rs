// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use httpmock::{
    Method::{GET, PUT},
    MockRef, MockServer,
};
use serde_json::{json, Map, Value};

use hawkbit::{MaintenanceWindow, Type};

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

    pub fn add_target(
        &self,
        name: &str,
        expected_config_data: Option<Value>,
        deployment: Option<Deployment>,
    ) -> Target {
        let key = format!("Key{}", name);
        let mut links = Map::new();

        let config_data = match expected_config_data {
            Some(expected_config_data) => {
                let config_path = self
                    .server
                    .url(format!("/DEFAULT/controller/v1/{}/configData", name));
                links.insert("configData".into(), json!({ "href": config_path }));

                let config_data = self.server.mock(|when, then| {
                    when.method(PUT)
                        .path(format!("/DEFAULT/controller/v1/{}/configData", name))
                        .header("Content-Type", "application/json")
                        .header("Authorization", &format!("TargetToken {}", key))
                        .json_body(expected_config_data);

                    then.status(200);
                });

                Some(config_data)
            }
            None => None,
        };

        let deployment = deployment.map(|deploy| {
            let deploy_path = self.server.url(format!(
                "/DEFAULT/controller/v1/{}/deploymentBase/{}",
                name, deploy.id
            ));
            links.insert("deploymentBase".into(), json!({ "href": deploy_path }));

            let response = deploy.json();

            self.server.mock(|when, then| {
                when.method(GET)
                    .path(format!(
                        "/DEFAULT/controller/v1/{}/deploymentBase/{}",
                        name, deploy.id
                    ))
                    .header("Authorization", &format!("TargetToken {}", key));

                then.status(200)
                    .header("Content-Type", "application/json")
                    .json_body(response);
            })
        });

        let response = json!({
            "config": {
                "polling": {
                    "sleep": "00:01:00"
                }
            },
            "_links": links
        });

        let poll = self.server.mock(|when, then| {
            when.method(GET)
                .path(format!("/{}/controller/v1/{}", self.tenant, name))
                .header("Authorization", &format!("TargetToken {}", key));

            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(response);
        });

        Target {
            name: name.to_string(),
            key,
            poll,
            config_data,
            deployment,
        }
    }
}

pub struct Target<'a> {
    pub name: String,
    pub key: String,
    poll: MockRef<'a>,
    config_data: Option<MockRef<'a>>,
    deployment: Option<MockRef<'a>>,
}

impl<'a> Target<'a> {
    pub fn poll_hits(&self) -> usize {
        self.poll.hits()
    }

    pub fn config_data_hits(&self) -> usize {
        self.config_data.as_ref().unwrap().hits()
    }

    pub fn deployment_hits(&self) -> usize {
        self.deployment.as_ref().unwrap().hits()
    }
}

pub struct DeploymentBuilder {
    id: String,
    download_type: Type,
    update_type: Type,
    maintenance_window: Option<MaintenanceWindow>,
}
pub struct Deployment {
    id: String,
    download_type: Type,
    update_type: Type,
    maintenance_window: Option<MaintenanceWindow>,
}

impl DeploymentBuilder {
    pub fn new(id: &str, download_type: Type, update_type: Type) -> Self {
        Self {
            id: id.to_string(),
            download_type,
            update_type,
            maintenance_window: None,
        }
    }

    pub fn maintenance_window(self, maintenance_window: MaintenanceWindow) -> Self {
        let mut builder = self;
        builder.maintenance_window = Some(maintenance_window);
        builder
    }

    // TODO: chunks

    pub fn build(self) -> Deployment {
        Deployment {
            id: self.id,
            download_type: self.download_type,
            update_type: self.update_type,
            maintenance_window: self.maintenance_window,
        }
    }
}

impl Deployment {
    fn json(&self) -> serde_json::Value {
        let mut j = json!({
            "id": self.id,
            "deployment": {
                "download": self.download_type,
                "update": self.update_type,
                "chunks": []
            }
        });

        if let Some(maintenance_window) = &self.maintenance_window {
            let d = j.get_mut("deployment").unwrap().as_object_mut().unwrap();
            d.insert("maintenanceWindow".to_string(), json!(maintenance_window));
        }

        j
    }
}
