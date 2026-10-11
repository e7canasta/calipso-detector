//! Single-slot, newest-wins mailbox between the ingest and engine threads.
//!
//! `put` never blocks and replaces any unconsumed value. The consumer blocks
//! in `take`; while it does, `is_waiting` lets the producer know a sample is
//! wanted right now, so expensive conversions happen only on demand.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

#[derive(Debug)]
pub enum Take<T> {
    Value(T),
    Timeout,
    Closed,
}

#[derive(Debug)]
pub struct Mailbox<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
}

#[derive(Debug)]
struct State<T> {
    value: Option<T>,
    waiting: bool,
    closed: bool,
    overwritten: u64,
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                value: None,
                waiting: false,
                closed: false,
                overwritten: 0,
            }),
            ready: Condvar::new(),
        }
    }
}

impl<T> Mailbox<T> {
    /// Stores `value`, dropping an unconsumed one. Returns `false` once closed.
    pub fn put(&self, value: T) -> bool {
        let mut state = self.state.lock().expect("mailbox poisoned");
        if state.closed {
            return false;
        }
        if state.value.replace(value).is_some() {
            state.overwritten += 1;
        }
        self.ready.notify_one();
        true
    }

    pub fn take(&self, timeout: Duration) -> Take<T> {
        let mut state = self.state.lock().expect("mailbox poisoned");
        state.waiting = true;
        let (mut state, _) = self
            .ready
            .wait_timeout_while(state, timeout, |state| {
                state.value.is_none() && !state.closed
            })
            .expect("mailbox poisoned");
        state.waiting = false;
        match state.value.take() {
            Some(value) => Take::Value(value),
            None if state.closed => Take::Closed,
            None => Take::Timeout,
        }
    }

    /// The consumer is blocked waiting for a value.
    pub fn is_waiting(&self) -> bool {
        self.state.lock().expect("mailbox poisoned").waiting
    }

    /// Values replaced before being taken.
    pub fn overwritten(&self) -> u64 {
        self.state.lock().expect("mailbox poisoned").overwritten
    }

    pub fn close(&self) {
        self.state.lock().expect("mailbox poisoned").closed = true;
        self.ready.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, thread};

    #[test]
    fn newest_value_wins() {
        let mailbox = Mailbox::default();
        mailbox.put(1);
        mailbox.put(2);
        assert!(matches!(mailbox.take(Duration::ZERO), Take::Value(2)));
        assert_eq!(mailbox.overwritten(), 1);
        assert!(matches!(mailbox.take(Duration::ZERO), Take::Timeout));
    }

    #[test]
    fn close_wakes_consumer() {
        let mailbox = Arc::new(Mailbox::<u32>::default());
        let consumer = {
            let mailbox = Arc::clone(&mailbox);
            thread::spawn(move || matches!(mailbox.take(Duration::from_secs(10)), Take::Closed))
        };
        while !mailbox.is_waiting() {
            thread::yield_now();
        }
        mailbox.close();
        assert!(consumer.join().unwrap());
        assert!(!mailbox.put(1));
    }
}
