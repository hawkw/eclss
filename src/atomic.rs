use std::fmt;
pub use std::sync::atomic::*;

#[derive(Default)]
pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    /// This is a separate function because `f32::to_bits` is not yet stable as
    /// a `const fn`, and we would like a const constructor.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self(AtomicU32::new(0))
    }

    #[inline]
    #[must_use]
    pub fn new(f: f32) -> Self {
        Self(AtomicU32::new(f.to_bits()))
    }

    #[inline]
    #[must_use]
    pub fn load(&self, order: Ordering) -> f32 {
        let bits = self.0.load(order);
        f32::from_bits(bits)
    }

    #[inline]
    pub fn store(&self, f: f32, order: Ordering) {
        let bits = f.to_bits();
        self.0.store(bits, order)
    }

    // TODO(eliza): add CAS, etc, if needed.
}

impl fmt::Debug for AtomicF32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AtomicF32")
            .field(&self.load(Ordering::Acquire))
            .finish()
    }
}
