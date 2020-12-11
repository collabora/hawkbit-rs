// Copyright 2020, Collabora Ltd.
// SPDX-License-Identifier: MIT

use anyhow::Result;
use hawkbit::DirectDeviceIntegration;
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

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    let ddi = DirectDeviceIntegration::new(&opt.url, &opt.tenant, &opt.controller, &opt.key)?;

    loop {
        let reply = ddi.poll().await?;
        dbg!(&reply);

        let t = reply.polling_sleep()?;
        delay_for(t).await;
    }
}
