#![feature(local_waker)]
#![feature(thread_sleep_until)]

use std::{future::Future, task::Poll, time::Duration};

use futures::AsyncReadExt;

pub mod io;
pub mod rt;

struct CounterFuture {
    target: usize,
    value: usize,
}

impl CounterFuture {
    fn new(target: usize) -> Self {
        Self { target, value: 0 }
    }
}

impl Future for CounterFuture {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        log::debug!("poll #{}", self.value);
        if self.value >= self.target {
            Poll::Ready(())
        } else {
            unsafe { self.get_unchecked_mut().value += 1 };
            cx.local_waker().wake_by_ref();
            Poll::Pending
        }
    }
}

async fn waow() -> usize {
    let counter = CounterFuture::new(10);
    counter.await;
    2
}

fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    rt::task::spawn(async {
        let mut stdin = io::stdin::open().expect("failed to open stdin");
        loop {
            let mut buffer = [0; 4096];
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

    rt::task::spawn(async {
        for i in 0..10 {
            rt::Timer::new(Duration::from_secs(1)).await;
            log::info!("cycle {}", i);
        }
    });

    log::info!("starting");

    let res = rt::executor::run();
    log::info!("executor exited with {:?}", res);
}
