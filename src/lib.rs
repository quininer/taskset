use std::pin::Pin;
use std::marker::Unpin;
use std::sync::{ Arc, Weak };
use std::task::{ Context, Poll };
use std::future::Future;
use slab::Slab;
use crossbeam_queue::SegQueue;
use futures_core::stream::Stream;
use futures_task::{ ArcWake, waker_ref };
use futures::task::AtomicWaker;


pub struct TaskSet<Fut> {
    ready_queue: Arc<ReadyQueue>,
    tasks: Slab<(Fut, Arc<TaskWaker>)>
}

struct ReadyQueue {
    queue: SegQueue<usize>,
    waker: AtomicWaker
}

struct TaskWaker {
    id: usize,
    ready_queue: Weak<ReadyQueue>
}

const YIELD_COUNT: usize = 32;

impl<Fut> Default for TaskSet<Fut> {
    fn default() -> TaskSet<Fut> {
        TaskSet {
            ready_queue: Arc::new(ReadyQueue {
                queue: SegQueue::new(),
                waker: AtomicWaker::new()
            }),
            tasks: Slab::new()
        }
    }
}

impl<Fut> TaskSet<Fut> {
    #[inline]
    pub fn new() -> TaskSet<Fut> {
        TaskSet::default()
    }

    pub fn push(&mut self, fut: Fut) {
        let entry = self.tasks.vacant_entry();
        let id = entry.key();
        let waker = Arc::new(TaskWaker {
            id,
            ready_queue: Arc::downgrade(&self.ready_queue)
        });
        entry.insert((fut, waker));
        self.ready_queue.queue.push(id);
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }
}

impl<Fut: Future + Unpin> Stream for TaskSet<Fut> {
    type Item = Fut::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // register waker
        this.ready_queue.waker.register(cx.waker());

        let mut polled = 0;

        while let Ok(id) = this.ready_queue.queue.pop() {
            if let Some((task, waker)) = this.tasks.get_mut(id) {
                let waker = waker_ref(waker);
                let mut cx = Context::from_waker(&waker);

                // poll task
                if let Poll::Ready(output) = Pin::new(task).poll(&mut cx) {
                    this.tasks.remove(id);
                    return Poll::Ready(Some(output));
                } else if polled >= YIELD_COUNT {
                    cx.waker().wake_by_ref();
                    return Poll::Pending
                } else {
                    polled += 1;
                }
            }
        }

        if this.tasks.is_empty() {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

impl ArcWake for TaskWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        if let Some(rq) = arc_self.ready_queue.upgrade() {
            // push task
            rq.queue.push(arc_self.id);

            // wake
            if let Some(waker) = rq.waker.take() {
                waker.wake();
            }
        }
    }
}
