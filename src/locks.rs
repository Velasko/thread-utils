use std::sync::{Arc, Condvar, Mutex, Weak};

use crate::child;

#[derive(Default)]
pub(crate) struct Alarm {
    this: Weak<Self>,
    lock: Mutex<bool>,
    buzzer: Condvar,
}

impl Alarm {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            lock: Mutex::new(false),
            buzzer: Condvar::new(),
        })
    }

    pub fn set(&self) {
        loop {
            match child::get_thread_pool() {
                None => (),
                Some(pool) => {
                    pool.thread_to_sleep(self.this.upgrade().unwrap().clone());
                }
            }

            match self.lock.lock() {
                Err(_) => todo!("handle poison"),
                Ok(ring) => {
                    if *self.buzzer.wait(ring).unwrap() {
                        return;
                    }
                }
            }
        }
    }

    pub fn buzz(&self) {
        match self.lock.lock() {
            Err(_) => todo!("handle poison"),
            Ok(mut ring) => {
                *ring = true;
                self.buzzer.notify_all();
            }
        }
    }
}
