use serde::{Deserialize, Serialize, Serializer};
use std::{
    fmt,
    sync::atomic::{AtomicU8, Ordering},
};

/// Represents the status of an I2C sensor.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Status {
    /// A sensor of this type has never been initialized. It is likely that a
    /// sensor of this type is not connected to the bus.
    Missing,

    /// The sensor is connected and healthy.
    Up,

    /// The sensor has previously been brought up successfully, but it is no
    /// longer healthy.
    // TODO(eliza): can we distinguish between I2C bus disconnection and other
    // errors?
    Down,
}

impl Status {
    fn from_u8(u: u8) -> Self {
        match u {
            u if u == Status::Missing as u8 => Status::Missing,
            u if u == Status::Up as u8 => Status::Up,
            u if u == Status::Down as u8 => Status::Down,
            // Weird status, assume missing?
            _ => Status::Missing,
        }
    }
}

pub struct StatusCell(AtomicU8);

impl StatusCell {
    pub const fn new() -> Self {
        Self(AtomicU8::new(Status::Missing as u8))
    }

    pub fn set_status(&self, status: Status) -> Status {
        let prev = self.0.swap(status as u8, Ordering::AcqRel);
        Status::from_u8(prev)
    }

    #[must_use]
    pub fn status(&self) -> Status {
        Status::from_u8(self.0.load(Ordering::Acquire))
    }
}

impl fmt::Debug for StatusCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StatusCell").field(&self.status()).finish()
    }
}

impl Serialize for StatusCell {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.status().serialize(serializer)
    }
}
