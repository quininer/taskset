use std::mem;
use std::pin::Pin;
use std::marker::Unpin;
use std::collections::VecDeque;
use std::sync::{ atomic, Arc, Weak };
use std::task::{ Context, Waker, Poll };
use std::future::Future;
use parking_lot::Mutex;
use slab::Slab;
use futures_core::stream::Stream;
use futures_task::{ ArcWake, waker_ref };


pub struct TaskSet<Fut> {
    dequeue: VecDeque<usize>,
    ready_queue: Arc<Mutex<ReadyQueue>>,
    tasks: Slab<(Fut, Arc<TaskWaker>)>
}

struct ReadyQueue {
    queue: Vec<usize>,
    waker: Option<Waker>
}

struct TaskWaker {
    id: usize,
    queued: atomic::AtomicBool,
    ready_queue: Weak<Mutex<ReadyQueue>>
}

impl<Fut> Default for TaskSet<Fut> {
    fn default() -> TaskSet<Fut> {
        TaskSet {
            dequeue: VecDeque::new(),
            ready_queue: Arc::new(Mutex::new(ReadyQueue {
                queue: Vec::new(),
                waker: None
            })),
            tasks: Slab::new()
        }
    }
}

impl<Fut> TaskSet<Fut> {
    #[inline]
    pub fn new() -> TaskSet<Fut> {
        TaskSet::default()
    }

    pub fn with_capacity(cap: usize) -> TaskSet<Fut> {
        TaskSet {
            dequeue: VecDeque::with_capacity(cap),
            ready_queue: Arc::new(Mutex::new(ReadyQueue {
                queue: Vec::with_capacity(cap),
                waker: None
            })),
            tasks: Slab::new()
        }
    }

    pub fn push(&mut self, fut: Fut) {
        let entry = self.tasks.vacant_entry();
        let id = entry.key();
        let waker = Arc::new(TaskWaker {
            id,
            queued: atomic::AtomicBool::new(true),
            ready_queue: Arc::downgrade(&self.ready_queue)
        });
        entry.insert((fut, waker));
        self.dequeue.push_back(id);
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

        {
            let mut rq = this.ready_queue.lock();

            // register waker
            if let Some(w) = rq.waker.as_mut() {
                if !w.will_wake(cx.waker()) {
                    *w = cx.waker().clone();
                }
            } else {
                rq.waker = Some(cx.waker().clone());
            }

            // task dequeue
            this.dequeue.extend(rq.queue.drain(..));
        }

        // panic bomb
        struct Bomb<'a, T> {
            id: usize,
            tasks: &'a mut Slab<T>
        }

        impl<'a, Fut> Drop for Bomb<'a, Fut> {
            #[inline]
            fn drop(&mut self) {
                self.tasks.remove(self.id);
            }
        }

        while let Some(id) = this.dequeue.pop_front() {
            let bomb = Bomb {
                id,
                tasks: &mut this.tasks
            };

            if let Some((task, waker)) = bomb.tasks.get_mut(id) {
                waker.queued.store(false, atomic::Ordering::SeqCst);

                let waker2 = waker_ref(waker);
                let mut cx = Context::from_waker(&waker2);

                // poll task
                match Pin::new(task).poll(&mut cx) {
                    Poll::Ready(output) => return Poll::Ready(Some(output)),
                    Poll::Pending => mem::forget(bomb)
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
        if !arc_self.queued.swap(true, atomic::Ordering::SeqCst) {
            if let Some(rq) = arc_self.ready_queue.upgrade() {
                let mut rq = rq.lock();

                rq.queue.push(arc_self.id);

                if let Some(waker) = rq.waker.take() {
                    waker.wake();
                }
            }
        }
    }
}
