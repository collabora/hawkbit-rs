// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::rc::Rc;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
};

use httpmock::{
    Method::{GET, POST, PUT},
    MockRef, MockRefExt, MockServer,
};
use serde_json::{json, Map, Value};

use hawkbit::{Execution, Finished, MaintenanceWindow, Type};

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
            server: Rc::new(MockServer::start()),
            tenant: self.tenant,
        }
    }
}

pub struct Server {
    pub tenant: String,
    server: Rc<MockServer>,
}

impl Server {
    pub fn base_url(&self) -> String {
        self.server.base_url()
    }

    pub fn add_target(&self, name: &str) -> Target {
        Target::new(name, &self.server, &self.tenant)
    }
}

pub struct Target {
    pub name: String,
    pub key: String,
    server: Rc<MockServer>,
    tenant: String,
    poll: Cell<usize>,
    config_data: RefCell<Option<PendingAction>>,
    deployment: RefCell<Option<PendingAction>>,
}

impl Target {
    fn new(name: &str, server: &Rc<MockServer>, tenant: &str) -> Self {
        let key = format!("Key{}", name);

        let poll = Self::create_poll(server, tenant, name, &key, None, None);
        Target {
            name: name.to_string(),
            key,
            server: server.clone(),
            tenant: tenant.to_string(),
            poll: Cell::new(poll),
            config_data: RefCell::new(None),
            deployment: RefCell::new(None),
        }
    }

    fn create_poll(
        server: &MockServer,
        tenant: &str,
        name: &str,
        key: &str,
        expected_config_data: Option<&PendingAction>,
        deployment: Option<&PendingAction>,
    ) -> usize {
        let mut links = Map::new();

        if let Some(pending) = expected_config_data {
            links.insert("configData".into(), json!({ "href": pending.path }));
        }
        if let Some(pending) = deployment {
            links.insert("deploymentBase".into(), json!({ "href": pending.path }));
        }

        let response = json!({
            "config": {
                "polling": {
                    "sleep": "00:01:00"
                }
            },
            "_links": links
        });

        let mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/{}/controller/v1/{}", tenant, name))
                .header("Authorization", &format!("TargetToken {}", key));

            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(response);
        });

        mock.id()
    }

    fn update_poll(&self) {
        let old = self.poll.replace(Self::create_poll(
            &self.server,
            &self.tenant,
            &self.name,
            &self.key,
            self.config_data.borrow().as_ref(),
            self.deployment.borrow().as_ref(),
        ));

        let mut old = MockRef::new(old, &self.server);
        old.delete();
    }

    pub fn request_config(&self, expected_config_data: Value) {
        let config_path = self
            .server
            .url(format!("/DEFAULT/controller/v1/{}/configData", self.name));

        let config_data = self.server.mock(|when, then| {
            when.method(PUT)
                .path(format!("/DEFAULT/controller/v1/{}/configData", self.name))
                .header("Content-Type", "application/json")
                .header("Authorization", &format!("TargetToken {}", self.key))
                .json_body(expected_config_data);

            then.status(200);
        });

        self.config_data.replace(Some(PendingAction {
            server: self.server.clone(),
            path: config_path,
            mock: config_data.id(),
        }));

        self.update_poll();
    }

    pub fn push_deployment(&self, deploy: Deployment) {
        let deploy_path = self.server.url(format!(
            "/DEFAULT/controller/v1/{}/deploymentBase/{}",
            self.name, deploy.id
        ));

        let base_url = self.server.url("/download");
        let response = deploy.json(&base_url);

        let deploy_mock = self.server.mock(|when, then| {
            when.method(GET)
                .path(format!(
                    "/DEFAULT/controller/v1/{}/deploymentBase/{}",
                    self.name, deploy.id
                ))
                .header("Authorization", &format!("TargetToken {}", self.key));

            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(response);
        });

        // Serve the artifacts
        for chunk in deploy.chunks.iter() {
            for (artifact, _md5, _sha1, _sha256) in chunk.artifacts.iter() {
                let file_name = artifact.file_name().unwrap().to_str().unwrap();
                let path = format!("/download/{}", file_name);

                self.server.mock(|when, then| {
                    when.method(GET)
                        .path(path)
                        .header("Authorization", &format!("TargetToken {}", self.key));

                    then.status(200).body_from_file(artifact.to_str().unwrap());
                });
            }
        }

        self.deployment.replace(Some(PendingAction {
            server: self.server.clone(),
            path: deploy_path,
            mock: deploy_mock.id(),
        }));

        self.update_poll();
    }

    pub fn expect_feedback(
        &self,
        deployment_id: &str,
        execution: Execution,
        finished: Finished,
        progress: Option<serde_json::Value>,
        details: Vec<&str>,
    ) -> MockRef<'_> {
        let progress = progress.unwrap_or(serde_json::Value::Null);

        self.server.mock(|when, then| {
            let expected = json!({
                "id": deployment_id,
                "status": {
                    "result": {
                        "progress": progress,
                        "finished": finished
                    },
                    "execution": execution,
                    "details": details,
                },
            });

            when.method(POST)
                .path(format!(
                    "/{}/controller/v1/{}/deploymentBase/{}/feedback",
                    self.tenant, self.name, deployment_id
                ))
                .header("Authorization", &format!("TargetToken {}", self.key))
                .header("Content-Type", "application/json")
                .json_body(expected);

            then.status(200);
        })
    }

    pub fn poll_hits(&self) -> usize {
        let mock = MockRef::new(self.poll.get(), &self.server);
        mock.hits()
    }

    pub fn config_data_hits(&self) -> usize {
        self.config_data.borrow().as_ref().map_or(0, |m| {
            let mock = MockRef::new(m.mock, &self.server);
            mock.hits()
        })
    }

    pub fn deployment_hits(&self) -> usize {
        self.deployment.borrow().as_ref().map_or(0, |m| {
            let mock = MockRef::new(m.mock, &self.server);
            mock.hits()
        })
    }
}

struct PendingAction {
    server: Rc<MockServer>,
    mock: usize,
    path: String,
}

impl Drop for PendingAction {
    fn drop(&mut self) {
        let mut mock = MockRef::new(self.mock, &self.server);
        mock.delete();
    }
}

pub struct DeploymentBuilder {
    id: String,
    download_type: Type,
    update_type: Type,
    maintenance_window: Option<MaintenanceWindow>,
    chunks: Vec<Chunk>,
}
pub struct Deployment {
    pub id: String,
    download_type: Type,
    update_type: Type,
    maintenance_window: Option<MaintenanceWindow>,
    chunks: Vec<Chunk>,
}

impl DeploymentBuilder {
    pub fn new(id: &str, download_type: Type, update_type: Type) -> Self {
        Self {
            id: id.to_string(),
            download_type,
            update_type,
            maintenance_window: None,
            chunks: Vec::new(),
        }
    }

    pub fn maintenance_window(self, maintenance_window: MaintenanceWindow) -> Self {
        let mut builder = self;
        builder.maintenance_window = Some(maintenance_window);
        builder
    }

    pub fn chunk(
        self,
        part: &str,
        version: &str,
        name: &str,
        artifacts: Vec<(PathBuf, &str, &str, &str)>,
    ) -> Self {
        let mut builder = self;

        let artifacts = artifacts
            .into_iter()
            .map(|(path, md5, sha1, sha256)| {
                assert!(path.exists());
                (path, md5.to_string(), sha1.to_string(), sha256.to_string())
            })
            .collect();

        let chunk = Chunk {
            part: part.to_string(),
            version: version.to_string(),
            name: name.to_string(),
            artifacts,
        };
        builder.chunks.push(chunk);

        builder
    }

    pub fn build(self) -> Deployment {
        Deployment {
            id: self.id,
            download_type: self.download_type,
            update_type: self.update_type,
            maintenance_window: self.maintenance_window,
            chunks: self.chunks,
        }
    }
}

pub struct Chunk {
    part: String,
    version: String,
    name: String,
    artifacts: Vec<(PathBuf, String, String, String)>, // (path, md5, sha1, sha256)
}

impl Chunk {
    fn json(&self, base_url: &str) -> serde_json::Value {
        let artifacts: Vec<serde_json::Value> = self
            .artifacts
            .iter()
            .map(|(path, md5, sha1, sha256)| {
                let meta = path.metadata().unwrap();
                let file_name = path.file_name().unwrap().to_str().unwrap();
                let download_url = format!("{}/{}", base_url, file_name);
                // TODO: the md5 url is not served by the http server
                let md5_url = format!("{}.MD5SUM", download_url);

                json!({
                    "filename": file_name,
                    "hashes": {
                        "sha1": sha1,
                        "md5": md5,
                        "sha256": sha256,
                    },
                    "size": meta.len(),
                    "_links": {
                        "download": {
                            "href": download_url,
                        },
                        "download-http": {
                            "href": download_url,
                        },
                        "md5sum": {
                            "href": md5_url,
                        },
                        "md5sum-http": {
                            "href": md5_url,
                        },
                    }
                })
            })
            .collect();

        json!({
            "part": self.part,
            "version": self.version,
            "name": self.name,
            "artifacts": artifacts,
        })
    }
}

impl Deployment {
    fn json(&self, base_url: &str) -> serde_json::Value {
        let chunks: Vec<serde_json::Value> = self.chunks.iter().map(|c| c.json(base_url)).collect();

        let mut j = json!({
            "id": self.id,
            "deployment": {
                "download": self.download_type,
                "update": self.update_type,
                "chunks": chunks,
            }
        });

        if let Some(maintenance_window) = &self.maintenance_window {
            let d = j.get_mut("deployment").unwrap().as_object_mut().unwrap();
            d.insert("maintenanceWindow".to_string(), json!(maintenance_window));
        }

        j
    }
}
