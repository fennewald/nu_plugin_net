// TODO: remove
#![feature(local_waker)]
#![feature(box_vec_non_null)]
#![feature(thread_sleep_until)]
#![feature(type_alias_impl_trait)]

mod net;

pub mod plugin;
pub mod rt;

pub mod channel;

async fn entry() -> anyhow::Result<()> {
    plugin::entry::serve_cli(net::Net).await
}

fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    rt::spawn("main", async {
        let res = entry().await;
        log::info!("plugin exited with {:#?}", res);
    });

    let res = rt::run();
    log::info!("executor exited with {:?}", res);
}
