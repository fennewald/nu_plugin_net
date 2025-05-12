#![feature(local_waker)]
#![feature(thread_sleep_until)]
#![feature(type_alias_impl_trait)]

use std::time::Duration;

mod net;

pub mod io;
pub mod plugin;
pub mod rt;

pub mod channel;

async fn entry() -> anyhow::Result<()> {
    rt::time::sleep(Duration::from_secs(2)).await;
    plugin::entry::serve_cli(net::Net).await
}

fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    rt::spawn(async {
        let res = entry().await;
        log::info!("plugin exited with {:#?}", res);
    });

    let res = rt::run();
    log::info!("executor exited with {:?}", res);
}
