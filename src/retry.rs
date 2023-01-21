use std::marker::PhantomData;

pub struct Retry<E, F = fn(&E) -> bool> {
    max_retries: usize,
    should_retry: F,
    target: &'static str,
    _error: PhantomData<fn(E)>,
}

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
