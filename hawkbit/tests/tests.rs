// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs::File;
use std::io::prelude::*;
use std::{path::PathBuf, time::Duration};

use bytes::Bytes;
use futures::prelude::*;
use hawkbit::ddi::{Client, Error, Execution, Finished, MaintenanceWindow, Mode, Type};
use serde::Serialize;
use serde_json::json;
use tempdir::TempDir;

use hawkbit_mock::ddi::{Deployment, DeploymentBuilder, Server, ServerBuilder, Target};

fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

fn add_target(server: &Server, name: &str) -> (Client, Target) {
    let target = server.add_target(name);

    let client = Client::new(
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

#[tokio::test]
async fn upload_config() {
    init();

    let server = ServerBuilder::default().build();
    let (client, target) = add_target(&server, "Target1");

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
    target.request_config(expected_config_data);

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

fn artifact_path() -> PathBuf {
    let mut test_artifact = PathBuf::new();
    test_artifact.push("tests");
    test_artifact.push("data");
    test_artifact.push("test.txt");

    test_artifact
}

fn get_deployment(valid_checksums: bool) -> Deployment {
    let test_artifact = artifact_path();

    let artifacts = if valid_checksums {
        vec![(
            test_artifact,
            "5eb63bbbe01eeed093cb22bb8f5acdc3",
            "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed",
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
        )]
    } else {
        vec![(test_artifact, "badger", "badger", "badger")]
    };

    DeploymentBuilder::new("10", Type::Forced, Type::Attempt)
        .maintenance_window(MaintenanceWindow::Available)
        .chunk("app", "1.0", "some-chunk", artifacts)
        .build()
}

#[tokio::test]
async fn deployment() {
    init();

    let server = ServerBuilder::default().build();
    let (client, target) = add_target(&server, "Target1");
    target.push_deployment(get_deployment(true));

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
    assert_eq!(update.chunks().count(), 1);

    // Check chunk
    let chunk = update.chunks().next().unwrap();
    assert_eq!(chunk.part(), "app");
    assert_eq!(chunk.version(), "1.0");
    assert_eq!(chunk.name(), "some-chunk");
    assert_eq!(chunk.artifacts().count(), 1);

    let art = chunk.artifacts().next().unwrap();
    assert_eq!(art.filename(), "test.txt");
    assert_eq!(art.size(), 11);

    let out_dir = TempDir::new("test-hawkbitrs").expect("Failed to create temp dir");
    let artifacts = update
        .download(out_dir.path())
        .await
        .expect("Failed to download update");

    // Check artifact
    assert_eq!(artifacts.len(), 1);
    let p = artifacts[0].file();
    assert_eq!(p.file_name().unwrap(), "test.txt");
    assert!(p.exists());

    #[cfg(feature = "hash-md5")]
    artifacts[0].check_md5().await.expect("invalid md5");
    #[cfg(feature = "hash-sha1")]
    artifacts[0].check_sha1().await.expect("invalid sha1");
    #[cfg(feature = "hash-sha256")]
    artifacts[0].check_sha256().await.expect("invalid sha256");
}

#[tokio::test]
async fn send_feedback() {
    init();

    let server = ServerBuilder::default().build();
    let deploy = get_deployment(true);
    let deploy_id = deploy.id.clone();
    let (client, target) = add_target(&server, "Target1");
    target.push_deployment(deploy);

    let reply = client.poll().await.expect("poll failed");
    let update = reply.update().expect("missing update");
    let update = update.fetch().await.expect("failed to fetch update info");

    // Send feedback without progress
    let mut mock = target.expect_feedback(
        &deploy_id,
        Execution::Proceeding,
        Finished::None,
        None,
        vec!["Downloading"],
    );
    assert_eq!(mock.hits(), 0);

    update
        .send_feedback(Execution::Proceeding, Finished::None, vec!["Downloading"])
        .await
        .expect("Failed to send feedback");
    assert_eq!(mock.hits(), 1);
    mock.delete();

    // Send feedback with progress
    let mut mock = target.expect_feedback(
        &deploy_id,
        Execution::Closed,
        Finished::Success,
        Some(json!({"awesome": true})),
        vec!["Done"],
    );
    assert_eq!(mock.hits(), 0);

    #[derive(Serialize)]
    struct Progress {
        awesome: bool,
    }
    let progress = Progress { awesome: true };

    update
        .send_feedback_with_progress(
            Execution::Closed,
            Finished::Success,
            Some(progress),
            vec!["Done"],
        )
        .await
        .expect("Failed to send feedback");
    assert_eq!(mock.hits(), 1);
    mock.delete();
}

#[tokio::test]
async fn config_then_deploy() {
    init();

    let server = ServerBuilder::default().build();
    let (client, target) = add_target(&server, "Target1");

    let reply = client.poll().await.expect("poll failed");
    assert!(reply.config_data_request().is_none());
    assert!(reply.update().is_none());

    // server requests config
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
    target.request_config(expected_config_data);

    let reply = client.poll().await.expect("poll failed");
    assert!(reply.config_data_request().is_some());
    assert!(reply.update().is_none());

    // server pushes an update
    target.push_deployment(get_deployment(true));

    let reply = client.poll().await.expect("poll failed");
    assert!(reply.config_data_request().is_some());
    assert!(reply.update().is_some());
}

#[tokio::test]
async fn download_stream() {
    init();

    let server = ServerBuilder::default().build();
    let (client, target) = add_target(&server, "Target1");

    target.push_deployment(get_deployment(true));
    let reply = client.poll().await.expect("poll failed");

    let update = reply.update().expect("missing update");
    let update = update.fetch().await.expect("failed to fetch update info");
    let chunk = update.chunks().next().unwrap();
    let art = chunk.artifacts().next().unwrap();

    async fn check_download(mut stream: Box<dyn Stream<Item = Result<Bytes, Error>> + Unpin>) {
        let mut downloaded: Vec<u8> = Vec::new();
        while let Some(b) = stream.next().await {
            downloaded.extend(b.unwrap().as_ref());
        }

        // Compare downloaded content with the actual file
        let mut art_file = File::open(&artifact_path()).expect("failed to open artifact");
        let mut expected = Vec::new();
        art_file
            .read_to_end(&mut expected)
            .expect("failed to read artifact");

        assert_eq!(downloaded, expected);
    }

    // Download artifact using the stream API
    let stream = art
        .download_stream()
        .await
        .expect("failed to get download stream");
    check_download(Box::new(stream)).await;

    cfg_if::cfg_if! {
        if #[cfg(feature = "hash-md5")] {
            let stream = art
                .download_stream_with_md5_check()
                .await
                .expect("failed to get download stream");
            check_download(Box::new(stream)).await;
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "hash-sha1")] {
            let stream = art
                .download_stream_with_sha1_check()
                .await
                .expect("failed to get download stream");
            check_download(Box::new(stream)).await;
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "hash-sha256")] {
            let stream = art
                .download_stream_with_sha256_check()
                .await
                .expect("failed to get download stream");
            check_download(Box::new(stream)).await;
        }
    }
}

#[cfg(feature = "hash-digest")]
#[tokio::test]
async fn wrong_checksums() {
    use assert_matches::assert_matches;
    use hawkbit::ddi::ChecksumType;

    init();

    let server = ServerBuilder::default().build();
    let (client, target) = add_target(&server, "Target1");

    target.push_deployment(get_deployment(false));
    let reply = client.poll().await.expect("poll failed");

    let update = reply.update().expect("missing update");
    let update = update.fetch().await.expect("failed to fetch update info");
    let chunk = update.chunks().next().unwrap();
    let art = chunk.artifacts().next().unwrap();

    let out_dir = TempDir::new("test-hawkbitrs").expect("Failed to create temp dir");
    let downloaded = art
        .download(out_dir.path())
        .await
        .expect("failed to download artifact");

    #[cfg(feature = "hash-md5")]
    assert_matches!(
        downloaded.check_md5().await,
        Err(Error::ChecksumError(ChecksumType::Md5))
    );
    #[cfg(feature = "hash-sha1")]
    assert_matches!(
        downloaded.check_sha1().await,
        Err(Error::ChecksumError(ChecksumType::Sha1))
    );
    #[cfg(feature = "hash-sha256")]
    assert_matches!(
        downloaded.check_sha256().await,
        Err(Error::ChecksumError(ChecksumType::Sha256))
    );

    cfg_if::cfg_if! {
        if #[cfg(feature = "hash-md5")] {
            let stream = art
                .download_stream_with_md5_check()
                .await
                .expect("failed to get download stream");
            let end = stream.skip_while(|b| future::ready(b.is_ok())).next().await;
            assert_matches!(end, Some(Err(Error::ChecksumError(ChecksumType::Md5))));
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "hash-sha1")] {
            let stream = art
                .download_stream_with_sha1_check()
                .await
                .expect("failed to get download stream");
            let end = stream.skip_while(|b| future::ready(b.is_ok())).next().await;
            assert_matches!(end, Some(Err(Error::ChecksumError(ChecksumType::Sha1))));
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "hash-sha256")] {
            let stream = art
                .download_stream_with_sha256_check()
                .await
                .expect("failed to get download stream");
            let end = stream.skip_while(|b| future::ready(b.is_ok())).next().await;
            assert_matches!(end, Some(Err(Error::ChecksumError(ChecksumType::Sha256))));
        }
    }
}
