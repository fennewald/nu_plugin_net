#![feature(local_waker)]
#![feature(thread_sleep_until)]

use std::time::Duration;

use futures::{AsyncReadExt, StreamExt};

pub mod io;
pub mod rt;

fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    rt::spawn(async {
        let mut stdin = io::stdin::open().expect("failed to open stdin");
        loop {
            let mut buffer = [0; 16];
            let len = stdin
                .read(&mut buffer)
                .await
                .expect("could not read from stdin");
            log::info!("read in {} bytes", len);
            let s = std::str::from_utf8(&buffer[0..len])
                .expect("invalid UTF-8")
                .trim();
            log::info!("<{s}");
            if s == "quit" {
                break;
            }
        }
    });

    rt::spawn(async {
        rt::time::interval(Duration::from_secs(1))
            .take(10)
            .enumerate()
            .for_each(|(i, _)| async move { log::info!("cycle {i}") })
            .await
    });

    log::info!("starting");

    let res = rt::run();
    log::info!("executor exited with {:?}", res);
}
