use std::sync::{Mutex, MutexGuard};

/// Taking a lock back from a thread that panicked while holding it.
///
/// A panic anywhere poisons every mutex that thread held, and `unwrap()` on the
/// next lock turns that into a second panic — one glitched frame took the whole
/// session down. The grid it guards is worth drawing from either way.
pub trait LockRecover<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T>;
}

impl<T> LockRecover<T> for Mutex<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn poisoned() -> Arc<Mutex<u32>> {
        let lock = Arc::new(Mutex::new(7));
        let held = Arc::clone(&lock);
        let _ = std::thread::spawn(move || {
            let _guard = held.lock().unwrap();
            panic!("the thread dies holding it");
        })
        .join();
        lock
    }

    #[test]
    fn a_poisoned_lock_still_hands_back_its_value() {
        let lock = poisoned();

        assert!(lock.lock().is_err());
        assert_eq!(*lock.lock_recover(), 7);
    }

    #[test]
    fn a_recovered_lock_can_be_written_through() {
        let lock = poisoned();

        *lock.lock_recover() = 9;

        assert_eq!(*lock.lock_recover(), 9);
    }

    #[test]
    fn a_healthy_lock_is_unaffected() {
        let lock = Mutex::new(3);

        assert_eq!(*lock.lock_recover(), 3);
    }
}
