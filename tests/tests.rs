// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::time::Duration;

use hawkbit::DirectDeviceIntegration;

mod mock_ddi;
use mock_ddi::{Server, ServerBuilder, Target};

fn add_target<'a>(server: &'a Server, name: &str) -> (DirectDeviceIntegration, Target<'a>) {
    let target = server.add_target(name);

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
    let server = ServerBuilder::default().tenant("my-tenant").build();
    let (client, target) = add_target(&server, "Target1");

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
