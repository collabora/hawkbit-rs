// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use std::path::Path;

use anyhow::Result;
use hawkbit::{DirectDeviceIntegration, Execution, Finished};
use serde::Serialize;
use structopt::StructOpt;
use tokio::time::delay_for;

#[derive(StructOpt, Debug)]
#[structopt(name = "polling example")]
struct Opt {
    url: String,
    controller: String,
    key: String,
    #[structopt(short, long, default_value = "DEFAULT")]
    tenant: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConfigData {
    #[serde(rename = "HwRevision")]
    hw_revision: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    let ddi = DirectDeviceIntegration::new(&opt.url, &opt.tenant, &opt.controller, &opt.key)?;

    loop {
        let reply = ddi.poll().await?;
        dbg!(&reply);

        if let Some(request) = reply.config_data_request() {
            println!("Uploading config data");
            let data = ConfigData {
                hw_revision: "1.0".to_string(),
            };

            request
                .upload(Execution::Closed, Finished::Success, None, data)
                .await?;
        }

        if let Some(update) = reply.update() {
            println!("Pending update");

            let update = update.fetch().await?;
            dbg!(&update);

            update
                .send_feedback(Execution::Proceeding, Finished::None, vec!["Downloading"])
                .await?;

            let artifacts = update.download(Path::new("./download/")).await?;
            dbg!(&artifacts);

            for artifact in artifacts {
                #[cfg(feature = "hash-md5")]
                artifact.check_md5()?;
                #[cfg(feature = "hash-sha1")]
                artifact.check_sha1()?;
                #[cfg(feature = "hash-sha256")]
                artifact.check_sha256()?;
            }

            update
                .send_feedback(Execution::Closed, Finished::Success, vec![])
                .await?;
        }

        let t = reply.polling_sleep()?;
        delay_for(t).await;
    }
}
