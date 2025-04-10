use std::{
    future::Future,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll, Waker},
    thread,
};

use futures::{future::FutureExt, task::waker_ref};

use crate::queue::Queue;
use crate::task::Task;

pub struct Pool {
    this: Weak<Self>,
    workers: Vec<thread::JoinHandle<()>>,
    queue: Arc<Queue<Arc<Task>>>,
}

impl Pool {
    pub fn new(thread_ammount: usize) -> Arc<Self> {
        let pool = Arc::new_cyclic(|pool_ref| Self {
            this: pool_ref.clone(),
            workers: (0..thread_ammount)
                .map(|_| thread::spawn(move || {}))
                .collect::<Vec<thread::JoinHandle<()>>>(),
            queue: Queue::default(),
        });

        pool
    }

    pub fn default() -> Arc<Self> {
        let core_count: usize = std::thread::available_parallelism().map_or(1, |num| num.get());
        Self::new(core_count)
    }

    pub fn insert_task(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.queue.clone(),
        });

        self.queue.push(task);
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

    async fn my_func() {
        println!("I am here!");
    }

    // #[test]
    // fn pool_test() {
    //     let pool = Pool::default();
    //     pool.insert_task(async {
    //         println!("howdy! -- pool");
    //         my_func().await;
    //         println!("done! -- pool");
    //     });
    //
    //     pool.insert_task(my_func());
    //
    //     let v = Arc::downgrade(&pool);
    //
    //     let s = v.upgrade();
    //
    //     s.expect("idk").run();
    // }

    #[test]
    fn pool_death() {
        let dropped_pool = {
            let pool = Pool::default();
            Arc::downgrade(&pool)
        };

        assert!(dropped_pool.upgrade().is_none());
    }

    #[test]
    fn pool_ref() {
        let arc_pool = {
            let pool = Pool::default();
            Arc::clone(&pool)
        };

        assert_eq!(Arc::strong_count(&arc_pool), 1);
    }
}
