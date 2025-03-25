use std::{
    borrow::Borrow,
    cell::RefCell,
    future::Future,
    pin::Pin,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex, Weak,
    },
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

use futures::{
    future::{BoxFuture, FutureExt},
    task::{waker_ref, ArcWake},
};

use crate::pool::Pool;

thread_local! {
    static POOL: RefCell<Weak<Pool>> = RefCell::new(Arc::downgrade(&Pool::new(0)));
}

pub fn get_thread_pool() -> Option<Arc<Pool>> {
    POOL.with_borrow(|pool| pool.upgrade())
}

pub fn thread_operation(pool: Weak<Pool>) {
    POOL.set(pool.clone());

    loop {
        pool.upgrade().map(|p| {
            let mut task = p.fetch_task();
            let waker = waker_ref(&task);
            let context = &mut Context::from_waker(&waker);

            task.poll_unpin(context);
        });
    }
}

struct TaskState {
    completed: bool,
    waker: Option<Waker>,
}

pub struct TaskManager {
    task_state: Arc<Mutex<TaskState>>,
}

impl Future for TaskManager {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut task_state = self.task_state.lock().unwrap();
        if task_state.completed {
            Poll::Ready(())
        } else {
            task_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl TaskManager {
    pub fn new(duration: Duration) -> Self {
        let task_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
        }));

        // Spawn the new thread
        let thread_task_state = task_state.clone();
        thread::spawn(move || {
            thread::sleep(duration);
            let mut task_state = thread_task_state.lock().unwrap();
            // Signal that the timer has completed and wake up the last
            // task on which the future was polled, if one exists.
            task_state.completed = true;
            if let Some(waker) = task_state.waker.take() {
                waker.wake()
            }
        });

        TaskManager { task_state }
    }
}

pub(crate) struct Task {
    /// In-progress future that should be pushed to completion.
    ///
    /// The `Mutex` is not necessary for correctness, since we only have
    /// one thread executing tasks at once. However, Rust isn't smart
    /// enough to know that `future` is only mutated from one thread,
    /// so we need to use the `Mutex` to prove thread-safety. A production
    /// executor would not need this, and could use `UnsafeCell` instead.
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    // Handle to place the task itself back onto the task queue.
    // task_sender: SyncSender<Arc<Task>>,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // Implement `wake` by sending this task back onto the task channel
        // so that it will be polled again by the executor.
        let cloned = arc_self.clone();
        // arc_self
        //     .task_sender
        //     .try_send(cloned)
        //     .expect("too many tasks queued");
    }
}

impl Task {
    pub(crate) fn new<O>(task: dyn Future<Output = O>) -> Self {
        Self {
            future: Mutex::new(task),
        }
    }
}
