use std::pin::Pin;
use std::task::Poll;
use tokio::sync::oneshot;
use futures_core::stream::Stream;
use futures_util::future;
use futures_util::stream::StreamExt;
use futures_taskset::TaskSet;


#[tokio::test]
async fn test_set() {
    let mut queue = TaskSet::default();

    let (tx0, rx) = oneshot::channel::<usize>();
    queue.push(rx);
    let (tx1, rx) = oneshot::channel();
    queue.push(rx);
    let (tx2, rx) = oneshot::channel();
    queue.push(rx);
    let (tx3, rx) = oneshot::channel();
    queue.push(rx);

    let (barrier_tx, barrier) = oneshot::channel();

    tokio::spawn(async move {
        tx2.send(2).unwrap();
        tx3.send(3).unwrap();

        barrier.await.unwrap();

        tx0.send(0).unwrap();
    });

    // assert 2
    let ret = queue.next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ret, 2);

    // assert 3
    future::poll_fn(|cx| {
        let ret = Pin::new(&mut queue).poll_next(cx);
        assert_eq!(Poll::Ready(Some(Ok(3))), ret);
        Poll::Ready(())
    }).await;

    // assert pending
    future::poll_fn(|cx| {
        let ret = Pin::new(&mut queue).poll_next(cx);
        assert!(ret.is_pending());
        Poll::Ready(())
    }).await;

    barrier_tx.send(()).unwrap();

    // assert 0
    let ret = queue.next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ret, 0);

    // assert pending
    future::poll_fn(|cx| {
        let ret = Pin::new(&mut queue).poll_next(cx);
        assert!(ret.is_pending());
        Poll::Ready(())
    }).await;

    tx1.send(1).unwrap();

    // assert 1
    let ret = queue.next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ret, 1);

    // assert end
    let ret = queue.next()
        .await;
    assert!(ret.is_none());
}
