// stolen from
// https://github.com/adafruit/Adafruit_PM25AQI/blob/master/Adafruit_PM25AQI.cpp
// except the bugs, which are my own :)
// and also the datasheet, which is extremely translated:
// https://cdn-shop.adafruit.com/product-files/4632/4505_PMSA003I_series_data_manual_English_V2.6.pdf
use core::fmt;
use embedded_hal::blocking::i2c;

#[derive(Debug)]
pub struct Pmsa003i<I> {
    i2c: I,
}

#[derive(Copy, Clone, Debug)]
pub struct Reading {
    /// Particulate concentrations in µg/𝑚3.
    pub concentrations: Concentrations,

    /// Counts of particles of various diameters in 0.1L of air.
    pub counts: ParticleCounts,

    /// The sensor version field.
    pub sensor_version: u8,
}

/// Particulate concentrations in µg/𝑚3.
///
/// This is a separate struct from [`ParticleCounts`] so that they can have
/// separate [`fmt::Display`] implementations.
#[derive(Copy, Clone, Debug)]
pub struct Concentrations {
    /// PM1.0 concentration in µg/𝑚3, under environmental atmospheric
    /// conditions.
    ///
    /// *Note*: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet. I am guessing this refers to humidity
    /// compensation?
    pub pm1_0: u16,
    /// PM1.0 concentration in µg/𝑚3, under standard atmospheric conditions.
    pub pm1_0_standard: u16,

    /// PM2.5 concentration in µg/𝑚3, under environmental atmospheric
    /// conditions.
    ///
    /// Note: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet...
    pub pm2_5: u16,
    /// PM2.5 concentration in µg/𝑚3, under standard atmospheric conditions.
    pub pm2_5_standard: u16,

    /// PM10.0 concentration in µg/𝑚3, under environmental atmospheric
    /// conditions.
    ///
    /// Note: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet...
    pub pm10_0: u16,
    /// PM10.0 concentration in µg/𝑚3, under standard atmospheric conditions.
    pub pm10_0_standard: u16,
}

/// Counts of particles of various diameters in 0.1L of air.
///
/// This is a separate struct from [`Concetrations`] so that they can have
/// separate [`fmt::Display`] implementations.
#[derive(Copy, Clone, Debug)]
pub struct ParticleCounts {
    /// Number of particles with diameter >= 0.3 µm in 0.1L of air.
    pub particles_0_3um: u16,
    /// Number of particles with diameter >= 0.5 µm in 0.1L of air.
    pub particles_0_5um: u16,
    /// Number of particles with diameter >= 1.0 µm in 0.1L of air.
    pub particles_1_0um: u16,
    /// Number of particles with diameter >= 2.5 µm in 0.1L of air.
    pub particles_2_5um: u16,
    /// Number of particles with diameter >= 5.0 µm in 0.1L of air.
    pub particles_5_0um: u16,
    /// Number of particles with diameter >= 10.0 µm in 0.1L of air.
    pub particles_10_0um: u16,
}

#[derive(Debug)]
pub enum Error<E> {
    /// An error occurred while reading from the I²C bus.
    I2c(E),
    /// The sum of the packet did not match the checksum.
    Checksum { sum: u16, checksum: u16 },
    /// The packet was missing the magic word.
    BadMagic(u16),
    /// The sensor sent an error code.
    ///
    /// **Note**: I couldn't find any documentation of what these error codes
    /// actually mean in the data sheet. I assume if it's non-zero, that's bad?
    ErrorCode(u8),
}

const MAGIC: u16 = 0x424d;
const PACKET_LEN: usize = 32;
const I2C_ADDR: u8 = 0x12;

impl<I> Pmsa003i<I> {
    #[must_use]
    pub const fn new(i2c: I) -> Self {
        Self { i2c }
    }
}

impl<I, E> Pmsa003i<I>
where
    I: i2c::Read<Error = E>,
{
    pub fn read(&mut self) -> Result<Reading, Error<E>> {
        let mut bytes = [0; PACKET_LEN];
        self.i2c
            .read(I2C_ADDR, &mut bytes[..])
            .map_err(Error::I2c)?;
        // reads a 16-bit word from `offset`
        macro_rules! words {
            [$offset:expr] => {
                u16::from_be_bytes([bytes[$offset], bytes[$offset + 1]])
            }
        }

        let magic = words![0];
        if magic != MAGIC {
            // you didn't say the magic words!
            return Err(Error::BadMagic(magic));
        }

        if bytes[29] != 0 {
            // byte 29 is an error code
            return Err(Error::ErrorCode(bytes[27]));
        }

        // last two bytes are the checksum so dont include them in the checksum.
        let sum = bytes[0..PACKET_LEN - 2]
            .iter()
            .map(|&byte| byte as u16)
            .sum();
        let checksum = words![PACKET_LEN - 2];
        if sum != checksum {
            return Err(Error::Checksum { sum, checksum });
        }

        // bytes 0 and 1 are the magic, which we already looked at
        // bytes 2 and 3 are the length field, which i don't get why they send,
        // because the data sheet also tells us how long the packet is lol

        // now we get to the good stuff:
        let reading = Reading {
            concentrations: Concentrations {
                pm1_0_standard: words![4],
                pm2_5_standard: words![6],
                pm10_0_standard: words![8],

                pm1_0: words![10],
                pm2_5: words![12],
                pm10_0: words![14],
            },

            counts: ParticleCounts {
                particles_0_3um: words![16],
                particles_0_5um: words![18],
                particles_1_0um: words![20],
                particles_2_5um: words![22],
                particles_5_0um: words![24],
                particles_10_0um: words![26],
            },

            // remaining bytes are version, error code (not documented lol), and
            // the checksum, which we already looked at
            sensor_version: bytes[28],
        };

        Ok(reading)
    }
}

// === impl Error ===

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Checksum { sum, checksum } => write!(
                f,
                "PMSA003I packet checksum did not match (expected {checksum}, got {sum})"
            ),
            Self::I2c(err) => write!(f, "PMSA003I I²C error: {err}"),
            Self::BadMagic(actual) => write!(
                f,
                "PMSA003I didn't say the magic word (expected {MAGIC:#x}. got {actual:#x})"
            ),
            Self::ErrorCode(code) => write!(f, "PMSA003I sent error code {code:#x}"),
        }
    }
}

// === impl Reading ===

impl fmt::Display for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            concentrations,
            counts,
            sensor_version: _,
        } = self;
        concentrations.fmt(f)?;
        f.write_str(if f.alternate() { "\n" } else { "; " })?;
        counts.fmt(f)?;
        Ok(())
    }
}

// === impl Concentrations ===

impl Concentrations {
    pub const UNIT: &str = "µg/𝑚3";
}

impl fmt::Display for Concentrations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNIT: &str = Concentrations::UNIT;
        let Self {
            pm1_0,
            pm1_0_standard,
            pm2_5,
            pm2_5_standard,
            pm10_0,
            pm10_0_standard,
        } = self;

        write!(f, "PM 1.0: {pm1_0} {UNIT}")?;
        if f.alternate() {
            write!(f, " ({pm1_0_standard} {UNIT} std)")?;
        }

        write!(f, ", PM 2.5: {pm2_5} {UNIT}")?;
        if f.alternate() {
            write!(f, " ({pm2_5_standard} {UNIT} std)")?;
        }

        write!(f, ", PM 10.0: {pm10_0} {UNIT}")?;
        if f.alternate() {
            write!(f, " ({pm10_0_standard} {UNIT} std)")?;
        }

        Ok(())
    }
}

// === impl ParticleCounts ===

impl ParticleCounts {
    pub const UNIT: &str = "/0.1L";
}

impl fmt::Display for ParticleCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNIT: &str = ParticleCounts::UNIT;
        const UM: &str = "µm";
        let Self {
            particles_0_3um,
            particles_0_5um,
            particles_1_0um,
            particles_2_5um,
            particles_5_0um,
            particles_10_0um,
        } = self;
        write!(
            f,
            "0.3{UM}: {particles_0_3um}{UNIT}, \
            0.5{UM}: {particles_0_5um}{UNIT}, \
            1.0{UM}: {particles_1_0um}{UNIT}, \
            2.5{UM}: {particles_2_5um}{UNIT}, \
            5.0{UM}: {particles_5_0um}{UNIT}, \
            10.0{UM}: {particles_10_0um}{UNIT}"
        )
    }
}
