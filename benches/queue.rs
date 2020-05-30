use std::thread;
use std::task::Poll;
use std::collections::VecDeque;
use criterion::{ criterion_main, criterion_group, Criterion, black_box };
use futures_oneshot as oneshot;
// use tokio::sync::oneshot;
// use futures_channel::oneshot;
use futures_util::future;
use futures_util::stream::StreamExt;
use futures_executor::block_on;


const NUM: usize = 10000;

fn bench_queue(c: &mut Criterion) {
    c.bench_function("taskset", |b| {
        use futures_taskset::TaskSet;

        b.iter(|| {
            let mut txs = VecDeque::with_capacity(NUM);
            let mut rxs = TaskSet::with_capacity(NUM / 2);

            for _ in 0..NUM {
                let (tx, rx) = oneshot::channel();
                txs.push_back(tx);
                rxs.push(rx);
            }

            thread::spawn(move || {
                while let Some(tx) = txs.pop_front() {
                    let _ = tx.send(black_box("hello"));
                }
            });

            block_on(future::poll_fn(move |cx| {
                loop {
                    if let Poll::Ready(None) = rxs.poll_next_unpin(cx) {
                        break
                    }
                }
                Poll::Ready(())
            }))
        });
    });

    c.bench_function("futures_unordered", |b| {
        use futures_util::stream::FuturesUnordered;

        b.iter(|| {
            let mut txs = VecDeque::with_capacity(NUM);
            let mut rxs = FuturesUnordered::new();

            for _ in 0..NUM {
                let (tx, rx) = oneshot::channel();
                txs.push_back(tx);
                rxs.push(rx);
            }

            thread::spawn(move || {
                while let Some(tx) = txs.pop_front() {
                    let _ = tx.send(black_box("hello"));
                }
            });

            block_on(future::poll_fn(move |cx| {
                loop {
                    if let Poll::Ready(None) = rxs.poll_next_unpin(cx) {
                        break
                    }
                }
                Poll::Ready(())
            }))
        });
    });
}


criterion_group!{
    name = queue;
    config = Criterion::default().sample_size(30);
    targets = bench_queue
}

criterion_main!(queue);
