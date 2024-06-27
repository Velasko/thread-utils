use std::{
    cell::RefCell,
    collections::vec_deque::VecDeque,
    sync::{Condvar, Mutex, RwLock},
};

pub struct Queue<T> {
    data: RwLock<RefCell<VecDeque<T>>>,
    notifier: Condvar,
    pop_lock: Mutex<()>,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            data: RwLock::new(RefCell::new(VecDeque::new())),
            notifier: Condvar::new(),
            pop_lock: Mutex::new(()),
        }
    }
}

impl<T> Queue<T> {
    pub fn push(&self, data: T) {
        match self.data.write() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut guard) => {
                let queue = guard.get_mut();
                queue.push_back(data);
                self.notifier.notify_one();
            }
        }
    }

    pub fn try_pop(&self) -> Option<T> {
        let data = match self.pop_lock.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(pop_guard) => {
                let queue_size = match self.data.read() {
                    Err(_) => unimplemented!("Poisoned lock"),
                    Ok(queue) => (*queue).borrow().len(),
                };

                if queue_size == 0 {
                    self.notifier.wait(pop_guard);
                }

                match self.data.write() {
                    Err(_) => unimplemented!("Poisoned lock"),
                    Ok(mut guard) => {
                        let queue = guard.get_mut();
                        queue.pop_front()
                    }
                }
            }
        };

        self.notifier.notify_one();
        data
    }

    pub fn pop(&self) -> T {
        loop {
            match self.try_pop() {
                None => (),
                Some(value) => {
                    return value;
                }
            }
        }
    }

    pub fn wake_up(&self) {
        self.notifier.notify_one();
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
