use std::borrow::BorrowMut;
use std::collections::HashSet;
use std::{
    cell::RefCell,
    sync::{Arc, Condvar, Mutex, Weak},
    thread::{current, ThreadId},
};

use crate::child;

pub(crate) struct Alarm {
    this: Weak<Self>,
    lock: Mutex<HashSet<ThreadId>>,
    using: RefCell<HashSet<ThreadId>>,
    buzzer: Condvar,
}

impl Alarm {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            lock: Mutex::new(HashSet::new()),
            using: RefCell::new(HashSet::new()),
            buzzer: Condvar::new(),
        })
    }

    pub fn set(&self) {
        match child::get_thread_pool() {
            None => (),
            Some(pool) => {
                if pool.within_pool() {
                    pool.thread_to_sleep(self.this.upgrade().unwrap().clone());
                }
            }
        }

        loop {
            match self.lock.lock() {
                Err(_) => todo!("handle poison"),
                Ok(ring) => {
                    let current = current().id();
                    self.using.borrow_mut().insert(current.clone());

                    if self.buzzer.wait(ring).unwrap().remove(&current) {
                        todo!("Improve this part. Also not liking how calling two times could make the thread go BRRRRR.");
                        let pool = child::get_thread_pool();
                        if pool
                            .as_ref()
                            .is_some_and(|pool| pool.within_pool() && pool.thread_asleep(&current))
                        {
                            pool.unwrap().wake_thread(&current);
                        } else {
                            self.using.borrow_mut().remove(&current);
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn buzz(&self) {
        self.wake(self.using.borrow().clone())
    }

    pub(crate) fn wake(&self, threads: HashSet<ThreadId>) {
        match self.lock.lock() {
            Err(_) => todo!("handle poison"),
            Ok(mut ring) => {
                *ring = threads.union(&*ring).map(|t| t.clone()).collect();
                self.buzzer.notify_all();
            }
        }
    }
}
