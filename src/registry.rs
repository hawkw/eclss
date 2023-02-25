use std::{
    cell::UnsafeCell,
    fmt,
    mem::MaybeUninit,
    ptr,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::*},
};

use serde::{
    ser::{SerializeMap, SerializeSeq},
    Serialize, Serializer,
};

/// A statically-constructed but dynamically initialized registry of up to
/// `SIZE` `T`-typed values.
pub struct Registry<T, const SIZE: usize> {
    values: [Slot<T>; SIZE],
    next: AtomicUsize,
}

/// A helper for serializing/formatting a registry of tuples as a "map" of
/// key-value pairs.
pub struct RegistryMap<K, V, const SIZE: usize>(Registry<(K, V), SIZE>);

struct Slot<T> {
    value: UnsafeCell<MaybeUninit<T>>,
    initialized: AtomicBool,
}

impl<T, const SIZE: usize> Registry<T, SIZE> {
    const NEW_SLOT: Slot<T> = Slot {
        value: UnsafeCell::new(MaybeUninit::uninit()),
        initialized: AtomicBool::new(false),
    };

    #[must_use]
    pub const fn new() -> Self {
        Self {
            values: [Self::NEW_SLOT; SIZE],
            next: AtomicUsize::new(0),
        }
    }

    pub fn register<'registry>(&'registry self, value: T) -> Result<&'registry T, T> {
        let idx = self.next.fetch_add(1, AcqRel);

        let Some(slot) = self.values.get(idx) else {
            return Err(value);
        };
        assert!(!slot.initialized.load(Acquire), "slot already initialized!");

        let init = unsafe {
            // Safety: we have exclusive access to the slot.
            let uninit = &mut *slot.value.get();
            ptr::write(uninit.as_mut_ptr(), value);
            uninit.assume_init_ref()
        };

        let _was_init = slot.initialized.swap(true, AcqRel);
        debug_assert!(
            !_was_init,
            "slot initialized while we were initializing it, wtf!"
        );

        // value initialized!
        Ok(init)
    }

    #[must_use]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.values[..self.len()].iter().filter_map(Slot::get)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.next.load(Acquire)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        SIZE
    }
}

unsafe impl<T: Send, const SIZE: usize> Send for Registry<T, SIZE> {}
unsafe impl<T: Sync, const SIZE: usize> Sync for Registry<T, SIZE> {}

impl<T, const SIZE: usize> fmt::Debug for Registry<T, SIZE>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T, const SIZE: usize> Serialize for Registry<T, SIZE>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for value in self.iter() {
            seq.serialize_element(value)?;
        }
        seq.end()
    }
}

// === impl RegistryMap ===

impl<K, V, const SIZE: usize> RegistryMap<K, V, SIZE> {
    #[must_use]
    pub const fn new() -> Self {
        Self(Registry::new())
    }

    #[inline]
    pub fn register<'registry>(
        &'registry self,
        key: K,
        value: V,
    ) -> Result<&'registry (K, V), (K, V)> {
        self.0.register((key, value))
    }

    #[must_use]
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &(K, V)> {
        self.0.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl<K, V, const SIZE: usize> fmt::Debug for RegistryMap<K, V, SIZE>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.iter().map(|&(ref k, ref v)| (k, v)))
            .finish()
    }
}

impl<K, V, const SIZE: usize> Serialize for RegistryMap<K, V, SIZE>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (key, value) in self.iter() {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

// === impl Slot ==

impl<T> Slot<T> {
    fn get(&self) -> Option<&T> {
        if !self.initialized.load(Acquire) {
            return None;
        }

        unsafe {
            // Safety: we just checked the bit that tracks whether this value
            // was initialized.
            Some((&*self.value.get()).assume_init_ref())
        }
    }
}
