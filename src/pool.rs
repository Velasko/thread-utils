use std::{
    any::Any,
    cell::RefCell,
    collections::hash_map::HashMap,
    panic::{catch_unwind, UnwindSafe},
    sync::{Arc, Mutex, Weak},
    thread,
};

use crate::child;
use crate::mapjoin::MapJoin;
use crate::queue::Queue;

type Task = Box<dyn FnOnce() -> anyhow::Result<()>>;
type Panic = Box<dyn Any + Send>;

pub struct Pool {
    this: Weak<Pool>,
    threads: Mutex<HashMap<thread::ThreadId, thread::JoinHandle<()>>>,

    tasks: Queue<Task>,
    blocked_tasks: Queue<Task>,
}

impl Pool {
    pub fn new(thread_ammount: usize) -> Arc<Self> {
        let pool = Arc::new_cyclic(|poll_ref| Pool {
            this: poll_ref.clone(),
            threads: Mutex::new(HashMap::new()),

            tasks: Queue::default(),
            blocked_tasks: Queue::default(),
        });

        for _ in 0..thread_ammount {
            pool.spawn();
        }

        pool
    }

    pub fn default() -> Arc<Self> {
        let core_count = match std::thread::available_parallelism() {
            Ok(value) => value.get(),
            Err(_) => 1,
        };
        Self::new(core_count)
    }

    pub(crate) fn fetch_task(&self) -> Task {
        self.tasks.pop()
    }

    fn spawn(&self) {
        let self_ref: Weak<Pool> = self.this.clone();
        let new_thread = thread::spawn(move || child::thread_operation(self_ref));
        match self.threads.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut map) => {
                if map.insert(new_thread.thread().id(), new_thread).is_some() {
                    panic!("New thread had the same id as previous one !")
                }
            }
        }
    }

    pub(crate) fn within_pool(&self) -> bool {
        let id_current = thread::current().id();
        match self.threads.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(map) => map.contains_key(&id_current),
        }
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
            self.tasks.push(Box::new(lambda));
        }

        MapJoin::new(map, args_size)
    }
}

unsafe impl Sync for Pool {}
unsafe impl Send for Pool {}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    // Thread tests
    #[test]
    fn thread_count() {
        let thread_ammount: usize = rand::thread_rng().gen::<usize>() % 16 + 1;

        let pool = Pool::new(thread_ammount);
        assert_eq!(pool.threads.lock().expect("batata").len(), thread_ammount);
    }

    #[test]
    fn access_pool_from_thread() {
        let pool = Pool::new(1);
        let equality = pool
            .map(
                |p| Arc::<Pool>::ptr_eq(&p, &child::get_thread_pool().unwrap()),
                vec![pool.clone()],
            )
            .join();

        assert!(equality[0].as_ref().unwrap());
    }

    #[test]
    fn simple_math_prob() {
        let pool = Pool::default();

        let math_func = |x: i32| (x * x + 4) / 33;
        let values = vec![1, 2, 3, 4, 7, 33];

        let correct = values
            .iter()
            .map(|val| math_func(*val))
            .collect::<Vec<i32>>();

        let threaded_math = pool
            .map(math_func, values)
            .join()
            .iter()
            .map(|ret| ret.as_ref().expect("trivial testing").clone())
            .collect::<Vec<_>>();

        assert_eq!(correct, threaded_math);
    }
}
