use std::{
    future::Future,
    sync::mpsc::{sync_channel, Receiver, SyncSender},
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread,
};

use futures::{future::FutureExt, task::waker_ref};

use crate::task::Task;

pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // Maximum number of tasks to allow queueing in the channel at once.
    // This is just to make `sync_channel` happy, and wouldn't be present in
    // a real executor.
    const MAX_QUEUED_TASKS: usize = 10_000;
    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);
    (Executor { ready_queue }, Spawner { task_sender })
}

/// `Spawner` spawns new futures onto the task channel.
#[derive(Clone)]
pub struct Spawner {
    pub task_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });
        self.task_sender
            .try_send(task)
            .expect("too many tasks queued");
    }
}

/// Task executor that receives tasks off of a channel and runs them.
pub struct Executor {
    pub ready_queue: Receiver<Arc<Task>>,
}

impl Executor {
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.recv() {
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

    async fn my_func() -> i64 {
        println!("I am here!");
        3
    }

    use super::*;
    #[test]
    fn main_test() {
        let (executor, spawner) = new_executor_and_spawner();

        // Spawn a task to print before and after waiting on a timer.
        spawner.spawn(async {
            println!("howdy!");
            // Wait for our timer future to complete after two seconds.
            let x = my_func().await;
            println!("done!");
            print!("result: {x}");
        });

        // Drop the spawner so that our executor knows it is finished and won't
        // receive more incoming tasks to run.
        drop(spawner);

        // Run the executor until the task queue is empty.
        // This will print "howdy!", pause, and then print "done!".
        executor.run();
    }
}
