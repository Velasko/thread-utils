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

    pub(crate) fn clone_queue(&self) -> Arc<Queue<Arc<Task>>> {
        self.queue.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_recursion::async_recursion;
    use async_std;
    use futures::future;
    use std::{env::var, mem, thread::JoinHandle, time::Duration};

    async fn aux() {
        async_std::task::sleep(Duration::from_millis(1000)).await;
    }

    // Function to emulate a constant flow of tasks
    #[async_recursion]
    async fn self_inserter(queue: Arc<Queue<Arc<Task>>>) {
        // println!("I am running on {:?}", thread::current().id());
        let func = self_inserter(queue.clone());
        let future = func.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: queue.clone(),
        });

        queue.push(task);
    }

    #[test]
    fn pool_death() {
        let (dropped_pool, workers) = {
            let pool = Pool::new(4);
            let workers = Arc::clone(&pool.workers);

            thread::sleep(Duration::from_millis(10));
            assert!(
                !workers.iter().any(|th| th.is_finished()),
                "There are dead threads from the get-go"
            );

            let self_inserting_future = self_inserter(pool.clone_queue());
            pool.insert_task(self_inserting_future);

            (Arc::downgrade(&pool), workers)
        };

        thread::sleep(Duration::from_millis(10));

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
    fn child_executing_task() {
        let workers = {
            let pool = Pool::new(4);

            let self_inserting_future = self_inserter(pool.clone_queue());
            pool.insert_task(self_inserting_future);

            thread::sleep(Duration::from_millis(1000));
            pool.workers.clone()
        };

        while !workers.iter().all(|th| th.is_finished()) {}
    }

    async fn testing_pool_access(global: Arc<Pool>) {
        assert!(child::get_thread_pool()
            .is_some_and(|local| Arc::as_ptr(&local) == Arc::as_ptr(&global)));
    }

    #[test]
    fn child_pool_access() {
        // Checks if child can access the pool
        let pool = Pool::new(1);

        let test = testing_pool_access(pool.clone());
        pool.insert_task(test);
        thread::sleep(Duration::from_millis(100));
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
