//! Circular history of values (the most recent at the end).

use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub struct History {
    buf: VecDeque<f64>,
    cap: usize,
}

impl History {
    pub fn new(cap: usize) -> History {
        let cap = cap.max(1);
        History {
            buf: VecDeque::with_capacity(cap),
            cap,
        }
    }

    pub fn push(&mut self, v: f64) {
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(v);
    }

    pub fn values(&self) -> Vec<f64> {
        self.buf.iter().copied().collect()
    }

    pub fn last(&self) -> Option<f64> {
        self.buf.back().copied()
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn set_capacity(&mut self, cap: usize) {
        self.cap = cap.max(1);
        while self.buf.len() > self.cap {
            self.buf.pop_front();
        }
    }

    /// Maximum value (0.0 if empty).
    pub fn max(&self) -> f64 {
        self.buf.iter().copied().fold(0.0, f64::max)
    }
}
