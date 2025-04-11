use std::{
    cell::RefCell,
    future::Future,
    sync::{Arc, Mutex, Weak},
    task::Context,
    thread,
    time::Duration,
};

use futures::task::waker_ref;

use crate::pool::Pool;
use crate::queue::Queue;
use crate::task::Task;

thread_local! {
    static POOL: RefCell<Option<Weak<Pool>>> = RefCell::new(None);
}

pub fn get_thread_pool() -> Option<Arc<Pool>> {
    POOL.with_borrow(|opt| opt.clone().map(|pool| pool.upgrade()))
        .flatten()
}

pub fn thread_operation(pool: Weak<Pool>) {
    POOL.set(Some(pool.clone()));

    let queue: Arc<Queue<Arc<Task>>> = {
        let mut q = None;
        while let None = q {
            q = match pool.upgrade() {
                None => {
                    // todo!("Improve the thread waiting for the pool to fully initialize")
                    thread::sleep(Duration::from_millis(10));
                    None
                }
                Some(p) => Some(p.clone_queue()),
            };
        }
        q.clone()
            .expect("Pool's child should've gotten the queue")
            .clone()
    };

    while pool.upgrade().is_some() {
        // sleep here makes the pool to never be dropped.
        let task = queue.pop();
        let mut future_slot = task.future.lock().unwrap();
        if let Some(mut future) = future_slot.take() {
            // Create a `LocalWaker` from the task itself
            let waker = waker_ref(&task);
            let context = &mut Context::from_waker(&waker);
            // `BoxFuture<T>` is a type alias for
            // `Pin<Box<dyn Future<Output = T> + Send + 'static>>`.
            // We can get a `Pin<&mut dyn Future + Send + 'static>`
            // from it by calling the `Pin::as_mut` method.
            if future.as_mut().poll(context).is_pending() {
                // We're not done processing the future, so put it
                // back in its task to be run again in the future.
                *future_slot = Some(future);
            }
        }
    }
}
