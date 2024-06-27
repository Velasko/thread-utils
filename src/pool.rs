use lazy_static::lazy_static;
use std::{
    any::Any,
    cell::RefCell,
    collections::{hash_map::HashMap, vec_deque::VecDeque},
    panic::{catch_unwind, UnwindSafe},
    sync::{Arc, Condvar, Mutex, OnceLock, RwLock, Weak},
    thread,
};

use crate::child;
use crate::locks::Alarm;
use crate::mapjoin::MapJoin;
use crate::queue::Queue;

type Task = Box<dyn FnOnce() -> anyhow::Result<()>>;
type Panic = Box<dyn Any + Send>;

type ThreadVec = Vec<thread::JoinHandle<()>>;

pub struct Pool {
    this: Weak<Pool>,
    tasks: Queue<Task>,
    waking_count: RwLock<usize>,

    threads: Mutex<ThreadVec>,
    blocked_threads: Mutex<HashMap<thread::ThreadId, Arc<Alarm>>>,
    waking_threads: Queue<Arc<Alarm>>,
}

impl Pool {
    pub fn new(thread_ammount: usize) -> Arc<Self> {
        Arc::new_cyclic(|poll_ref| {
            let pool = Pool {
                this: poll_ref.clone(),
                tasks: Queue::default(),
                waking_count: RwLock::new(0),

                threads: Mutex::new(ThreadVec::new()),
                blocked_threads: Mutex::new(HashMap::new()),
                waking_threads: Queue::default(),
            };

            for _ in 0..thread_ammount {
                pool.spawn();
            }

            pool
        })
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

    pub(crate) fn any_thread_waking_up(&self) -> bool {
        self.waking_count.read().is_ok_and(|counter| *counter != 0)
    }

    pub(crate) fn get_alarm(&self) -> Arc<Alarm> {
        let thread_id = thread::current().id();
        let alarm = Arc::new(Alarm::default());

        match self.blocked_threads.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut alarms) => {
                alarms.insert(thread_id, alarm.clone());
            }
        }

        alarm
    }

    pub(crate) fn wake_thread(&self, id: &thread::ThreadId) {
        let data = match self.blocked_threads.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut map) => map.remove(id),
        };

        if let Some(alarm) = data {
            match self.waking_count.write() {
                Err(_) => unimplemented!("Poisoned lock"),
                Ok(mut counter) => {
                    *counter += 1;
                    self.waking_threads.push(alarm);
                }
            }
        }
    }

    pub(crate) fn restart_thread(&self) -> bool {
        match self.waking_count.write() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut counter) => {
                if *counter == 0 {
                    false
                } else {
                    *counter -= 1;
                    let alarm = self.waking_threads.pop();
                    alarm.buzz();
                    true
                }
            }
        }
    }

    fn spawn(&self) {
        let self_ref: Arc<Pool> = self.this.clone().upgrade().expect("Cloning ref to self");
        let new_thread = thread::spawn(move || child::thread_operation(self_ref));
        match self.threads.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut vec) => vec.push(new_thread),
        }
    }

    pub(crate) fn remove_self(&self) {
        let thread_id = thread::current().id();
        match self.threads.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut vec) => vec.retain(|handle| handle.thread().id() != thread_id),
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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use rand::Rng;
//
//     // Thread tests
//     #[test]
//     fn thread_count() {
//         let thread_ammount: usize = rand::thread_rng().gen::<usize>() % 16 + 1;
//         let mut pool = Pool::new(thread_ammount);
//         assert_eq!(pool.threads.len(), thread_ammount);
//     }
// }
