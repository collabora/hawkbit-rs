// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::time::Duration;

use hawkbit::{DirectDeviceIntegration, Execution, Finished, MaintenanceWindow, Mode, Type};
use serde::Serialize;
use serde_json::{json, Value};

mod mock_ddi;
use mock_ddi::{Deployment, DeploymentBuilder, Server, ServerBuilder, Target};

fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

fn add_target<'a>(
    server: &'a Server,
    name: &str,
    expected_config_data: Option<Value>,
    deployment: Option<Deployment>,
) -> (DirectDeviceIntegration, Target<'a>) {
    let target = server.add_target(name, expected_config_data, deployment);

    let client = DirectDeviceIntegration::new(
        &server.base_url(),
        &server.tenant,
        &target.name,
        &target.key,
    )
    .expect("DDI creation failed");

    (client, target)
}

#[tokio::test]
async fn poll() {
    init();

    let server = ServerBuilder::default().tenant("my-tenant").build();
    let (client, target) = add_target(&server, "Target1", None, None);

    assert_eq!(target.poll_hits(), 0);

    // Try polling twice
    for i in 0..2 {
        let reply = client.poll().await.expect("poll failed");
        assert_eq!(reply.polling_sleep().unwrap(), Duration::from_secs(60));
        assert!(reply.config_data_request().is_none());
        assert!(reply.update().is_none());
        assert_eq!(target.poll_hits(), i + 1);
    }
}

#[tokio::test]
async fn upload_config() {
    init();

    let server = ServerBuilder::default().build();
    let expected_config_data = json!({
        "mode" : "merge",
        "data" : {
            "awesome" : true,
        },
        "status" : {
            "result" : {
            "finished" : "success"
            },
            "execution" : "closed",
            "details" : [ "Some stuffs" ]
        }
    });
    let (client, target) = add_target(&server, "Target1", Some(expected_config_data), None);

    let reply = client.poll().await.expect("poll failed");
    let config_data_req = reply
        .config_data_request()
        .expect("missing config data request");
    assert!(reply.update().is_none());

    #[derive(Serialize)]
    struct Config {
        awesome: bool,
    }

    let config = Config { awesome: true };

    config_data_req
        .upload(
            Execution::Closed,
            Finished::Success,
            Some(Mode::Merge),
            config,
            vec!["Some stuffs"],
        )
        .await
        .expect("upload config failed");

    assert_eq!(target.poll_hits(), 1);
    assert_eq!(target.config_data_hits(), 1);
}

#[tokio::test]
async fn deployment() {
    init();

    let server = ServerBuilder::default().build();
    let deployment = DeploymentBuilder::new("10", Type::Forced, Type::Attempt)
        .maintenance_window(MaintenanceWindow::Available)
        .build();
    let (client, target) = add_target(&server, "Target1", None, Some(deployment));

    let reply = client.poll().await.expect("poll failed");
    assert!(reply.config_data_request().is_none());
    assert_eq!(target.deployment_hits(), 0);

    let update = reply.update().expect("missing update");
    let update = update.fetch().await.expect("failed to fetch update info");
    assert_eq!(target.deployment_hits(), 1);
    assert_eq!(update.download_type(), Type::Forced);
    assert_eq!(update.update_type(), Type::Attempt);
    assert_eq!(
        update.maintenance_window(),
        Some(MaintenanceWindow::Available)
    );
    assert_eq!(update.chunks().count(), 0);
}
