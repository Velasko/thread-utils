use std::{
    cell::RefCell,
    collections::vec_deque::VecDeque,
    sync::{Condvar, Mutex},
};

pub struct Queue<T> {
    data: Mutex<RefCell<VecDeque<T>>>,
    notifier: Condvar,
    pop_lock: Mutex<()>,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            data: Mutex::new(RefCell::new(VecDeque::new())),
            notifier: Condvar::new(),
            pop_lock: Mutex::new(()),
        }
    }
}

impl<T> Queue<T> {
    pub fn push(&self, data: T) {
        match self.data.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut guard) => {
                let queue = guard.get_mut();
                queue.push_back(data);
                self.notifier.notify_one();
            }
        }
    }

    pub fn pop(&self) -> T {
        let data = {
            loop {
                match self.pop_lock.lock() {
                    Err(_) => unimplemented!("Poisoned lock"),
                    Ok(pop_guard) => {
                        match self.notifier.wait(pop_guard) {
                            Err(_) => unimplemented!("Poisoned lock"),
                            Ok(_) => {
                                match self.data.lock() {
                                    Err(_) => unimplemented!("Poisoned lock"),
                                    Ok(mut guard) => {
                                        let queue = guard.get_mut();
                                        match queue.pop_front() {
                                            None => (), // spurious wakeup
                                            Some(item) => {
                                                break item;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        self.notifier.notify_one();
        data
    }
}

unsafe impl<T> Sync for Queue<T> {}
unsafe impl<T> Send for Queue<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn queue_fifo() {
        let mut rng = rand::thread_rng();
        let queue = Queue::default();
        let a = rng.gen::<usize>();
        let b = rng.gen::<usize>();
        queue.push(a);
        queue.push(b);

        println!("a {:?}", queue.data);

        assert_eq!(queue.pop(), a);
        assert_eq!(queue.pop(), b);
    }
}
