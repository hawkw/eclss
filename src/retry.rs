use embassy_time::{Duration, Timer};
use std::marker::PhantomData;

#[derive(Copy, Clone, Debug)]
pub struct ExpBackoff {
    max: Duration,
    initial: Duration,
    current: Duration,
    target: &'static str,
}

pub struct Retry<E, F = fn(&E) -> bool> {
    max_retries: usize,
    should_retry: F,
    target: &'static str,
    _error: PhantomData<fn(E)>,
}

// === impl ExpBackoff ===

impl ExpBackoff {
    const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);

    pub const fn new(initial: Duration) -> Self {
        Self {
            max: Self::DEFAULT_MAX_BACKOFF,
            current: initial,
            initial,
            target: "retry",
        }
    }

    pub const fn with_max(self, max: Duration) -> Self {
        Self { max, ..self }
    }

    pub const fn with_target(self, target: &'static str) -> Self {
        Self { target, ..self }
    }

    pub fn wait(&mut self) -> Timer {
        log::debug!(target: self.target, "backing off for {}...", self.current);
        let current = self.current;

        if self.current < self.max {
            self.current *= 2;
        }

        Timer::after(current)
    }

    pub fn reset(&mut self) {
        log::debug!(target: self.target, "reset backoff to {}", self.initial);
        self.current = self.initial;
    }

    pub fn current(&self) -> Duration {
        self.current
    }
}

// === impl Retry ===

impl<E> Retry<E> {
    pub const fn new(max_retries: usize) -> Self {
        Self {
            max_retries,
            should_retry: |_: &E| true,
            target: "retry",
            _error: PhantomData,
        }
    }

    pub const fn with_target(self, target: &'static str) -> Self {
        Self {
            max_retries: self.max_retries,
            should_retry: self.should_retry,
            target,
            _error: PhantomData,
        }
    }
}

impl<E, F> Retry<E, F>
where
    F: Fn(&E) -> bool,
    E: std::fmt::Debug,
{
    pub fn with_predicate<F2>(self, should_retry: F2) -> Retry<E, F2>
    where
        F2: Fn(&E) -> bool,
    {
        Retry {
            max_retries: self.max_retries,
            should_retry,
            target: self.target,
            _error: PhantomData,
        }
    }

    pub fn run<T>(&self, mut op: impl FnMut() -> Result<T, E>) -> Result<T, E> {
        let mut retries = self.max_retries;
        loop {
            match op() {
                Ok(val) => return Ok(val),
                Err(error) if (self.should_retry)(&error) && retries > 0 => {
                    retries -= 1;
                    log::warn!(target: self.target, "retrying: {error:?} ({retries} retries remaining)");
                }
                Err(error) => return Err(error),
            }
        }
    }
}
