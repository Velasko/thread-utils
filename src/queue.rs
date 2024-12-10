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

    pub fn pop(&self) -> T {
        let data = match self.pop_lock.lock() {
            Err(_) => unimplemented!("Poisoned lock"),
            Ok(mut pop_guard) => {
                while self
                    .data
                    .read()
                    .map_or(true, |queue| (*queue).borrow().len() == 0)
                {
                    pop_guard = match self.notifier.wait(pop_guard) {
                        Ok(value) => value,
                        Err(value) => value.into_inner(),
                    };
                }

                match self.data.write() {
                    Err(_) => unimplemented!("Poisoned lock"),
                    Ok(mut guard) => {
                        let queue = guard.get_mut();
                        queue
                            .pop_front()
                            .expect("Queue should wait to have something before popping")
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
