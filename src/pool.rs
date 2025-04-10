use std::{
    future::Future,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll, Waker},
    thread,
};

use futures::{future::FutureExt, task::waker_ref};

use crate::child;
use crate::queue::Queue;
use crate::task::Task;

pub struct Pool {
    this: Weak<Self>,
    workers: Arc<Vec<thread::JoinHandle<()>>>,
    queue: Arc<Queue<Arc<Task>>>,
}

impl Pool {
    pub fn new(thread_ammount: usize) -> Arc<Self> {
        Arc::new_cyclic(|pool_ref| {
            let mut pool = Self {
                this: pool_ref.clone(),
                workers: Arc::new(vec![]),
                queue: Queue::default(),
            };

            for _ in 0..thread_ammount {
                pool.spawn_child();
            }

            pool
        })
    }

    pub fn default() -> Arc<Self> {
        let core_count: usize = std::thread::available_parallelism().map_or(1, |num| num.get());
        Self::new(core_count)
    }

    fn spawn_child(&mut self) {
        let self_ref: Weak<Pool> = self.this.clone();
        let new_thread = thread::spawn(move || child::thread_operation(self_ref));
        Arc::get_mut(&mut self.workers).unwrap().push(new_thread);
    }

    pub fn insert_task(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.queue.clone(),
        });

        self.queue.push(task);
    }

    pub(crate) fn fetch_task(&self) -> Arc<Task> {
        self.queue.pop()
    }

    pub fn run(&self) {
        while let task = self.queue.pop() {
            // Take the future, and if it has not yet completed (is still Some),
            // poll it in an attempt to complete it.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{mem, thread::JoinHandle, time::Duration};

    #[test]
    fn pool_death() {
        let (dropped_pool, workers) = {
            let pool = Pool::new(4);
            let workers = Arc::clone(&pool.workers);

            thread::sleep(Duration::from_millis(1000));
            assert!(
                !workers.iter().any(|th| th.is_finished()),
                "There are dead threads from the get-go"
            );

            (Arc::downgrade(&pool), workers)
        };

        thread::sleep(Duration::from_millis(10000));

        assert!(
            dropped_pool.upgrade().is_none(),
            "Pool reference isn't weak"
        );
        assert!(
            workers.iter().all(|th| th.is_finished()),
            "Some threads are still alive"
        );
    }

    #[test]
    fn pool_ref() {
        let arc_pool = {
            let pool = Pool::new(0);
            Arc::clone(&pool)
        };

        assert_eq!(Arc::strong_count(&arc_pool), 1);
    }
}
