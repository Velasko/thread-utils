use std::{
    any::Any,
    cell::RefCell,
    collections::hash_map::HashMap,
    ops::Deref,
    panic::{catch_unwind, UnwindSafe},
    sync::{Arc, Mutex},
    thread,
};

use crate::queue::Queue;

pub struct MapJoin<S> {
    map: Arc<Mutex<RefCell<HashMap<usize, S>>>>,
    args_size: usize,
}

impl<S> MapJoin<S> {
    pub fn join(&self) -> Vec<S> {
        loop {
            match self.map.lock() {
                Err(_) => panic!("Poison Error! Either thread pool panic or panic::catch_unwind did not catch user's function panic."),
                Ok(return_map) => {
                    if return_map.deref().borrow().len() >= self.args_size {
                        let mut rmap = return_map.take();
                        return (0..self.args_size)
                            .map(|key| rmap.remove(&key).unwrap())
                            .collect::<Vec<S>>();
                    };
                }
            }
        }
    }
}

type Task = Box<dyn FnOnce() -> anyhow::Result<()>>;
type Panic = Box<dyn Any + Send>;

fn thread_operation(queue: Arc<Queue<Task>>) -> ! {
    loop {
        let func = queue.pop();
        let _ = func();
    }
}

pub struct Pool {
    queue: Arc<Queue<Task>>,
    threads: Vec<thread::JoinHandle<()>>,
    thread_ids: Arc<Vec<thread::ThreadId>>,
    daemon: bool,
}

impl Default for Pool {
    fn default() -> Self {
        let core_count = match std::thread::available_parallelism() {
            Ok(value) => value.get(),
            Err(_) => 1,
        };
        Self::new(core_count)
    }
}

impl Pool {
    pub fn new(thread_ammount: usize) -> Self {
        let queue = Arc::new(Queue::default());
        let mut thread_ids = vec![thread::current().id()];
        let threads = (0..thread_ammount)
            .map(|_| {
                let rqueue = Arc::clone(&queue);
                let th = thread::spawn(move || thread_operation(rqueue));
                thread_ids.push(th.thread().id());
                th
            })
            .collect::<Vec<thread::JoinHandle<()>>>();

        Pool {
            queue,
            threads,
            thread_ids: Arc::new(thread_ids),
            daemon: false,
        }
    }

    pub fn is_daemon(&mut self, value: bool) {
        self.daemon = value;
    }

    pub fn map<T: 'static + UnwindSafe, S: 'static>(
        &self,
        func: fn(T) -> S,
        args: Vec<T>,
    ) -> MapJoin<Result<S, Panic>> {
        let map = Arc::new(Mutex::new(RefCell::new(HashMap::new())));

        let func = Arc::new(func);

        let args_size = args.len();

        for (n, arg) in args.into_iter().enumerate() {
            let rmap = Arc::clone(&map);
            let rfunc = Arc::clone(&func);
            let lambda = move || -> anyhow::Result<()> {
                let ret = catch_unwind(|| rfunc(arg));
                loop {
                    if let Ok(mut return_map) = rmap.lock() {
                        return_map.get_mut().insert(n, ret);
                        return Ok(());
                    };
                }
            };
            self.queue.push(Box::new(lambda));
        }

        MapJoin { map, args_size }
    }

    pub fn become_worker(&self) -> ! {
        let rqueue = Arc::clone(&self.queue);
        thread_operation(rqueue)
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        if self.daemon {
            self.become_worker();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    // Thread tests
    #[test]
    fn thread_count() {
        let thread_ammount: usize = rand::thread_rng().gen::<usize>() % 16 + 1;
        let mut pool = Pool::new(thread_ammount);
        assert_eq!(pool.threads.len(), thread_ammount);
    }
}
