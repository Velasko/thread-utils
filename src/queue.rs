use std::{
    cell::RefCell,
    collections::vec_deque::VecDeque,
    sync::{Condvar, Mutex},
    time::Duration,
};

pub struct Queue<T> {
    data: Mutex<RefCell<VecDeque<T>>>,
    pop_lock: Condvar,
    timeout: Duration,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self::new(100)
    }
}

impl<T> Queue<T> {
    pub fn new(timeout: u64) -> Self {
        Self {
            data: Mutex::new(RefCell::new(VecDeque::new())),
            pop_lock: Condvar::new(),
            timeout: Duration::from_millis(timeout),
        }
    }

    pub fn push(&self, data: T) {
        let mut binding = self.data.lock().unwrap();
        let queue = (*binding).get_mut();
        queue.push_back(data);
        self.pop_lock.notify_one();
    }

    pub fn pop(&self) -> T {
        let data = {
            loop {
                let lock = self.data.lock().unwrap();
                let (mut binding, _) = self.pop_lock.wait_timeout(lock, self.timeout).unwrap();
                let queue = (*binding).get_mut();
                if let Some(item) = queue.pop_front() {
                    break item;
                }
            }
        };
        self.pop_lock.notify_one();
        data
    }

    pub fn notify_one(&self) {
        self.pop_lock.notify_one()
    }

    pub fn notify_all(&self) {
        self.pop_lock.notify_all()
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
