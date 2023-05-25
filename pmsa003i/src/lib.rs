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
    /// PM1.0 concentration in µg/𝑚3.
    pub pm1_0_standard: u16,
    /// PM2.5 concentration in µg/𝑚3.
    pub pm2_5_standard: u16,
    /// PM10.0 concentration in µg/𝑚3.
    pub pm10_0_standard: u16,

    /// PM1.0 concentration in µg/𝑚3, under atmospheric environment.
    ///
    /// *Note*: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet. I am guessing this refers to humidity
    /// compensation?
    pub pm1_0: u16,
    /// PM2.5 concentration in µg/𝑚3, under atmospheric environment.
    ///
    /// Note: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet...
    pub pm2_5: u16,
    /// PM10.0 concentration in µg/𝑚3, under atmospheric environment.
    ///
    /// Note: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet...
    pub pm10_0: u16,

    /// Number of particles with diameter >= 0.3 µm in 0.1L of air.
    pub particles_0_3um: u16,
    /// Number of particles with diameter >= 0.5 µm in 0.1L of air.
    pub particles_0_5um: u16,
    /// Number of particles with diameter >= 2.5 µm in 0.1L of air.
    pub particles_2_5um: u16,
    /// Number of particles with diameter >= 5.0 µm in 0.1L of air.
    pub particles_5_0um: u16,
    /// Number of particles with diameter >= 10.0 µm in 0.1L of air.
    pub particles_10_0um: u16,
}

#[derive(Debug)]
pub enum Error<E> {
    I2c(E),
    Checksum { sum: u16, checksum: u16 },
    NoMagic,
}

impl<I, E> Pmsa003i<I>
where
    I: i2c::Read<Error = E>,
{
    pub fn read(&mut self) -> Result<Reading, Error<E>> {
        const MAGIC: u16 = 0x424d;
        const PACKET_LEN: usize = 32;
        const I2C_ADDR: u8 = 0x12;

        let mut buf = [0; PACKET_LEN];
        self.i2c.read(I2C_ADDR, &mut buf[..]).map_err(Error::I2c)?;

        if u16::from_be_bytes([buf[0], buf[1]]) != MAGIC {
            // magic isn't real :(
            return Err(Error::NoMagic);
        }

        // last two bytes are the checksum so dont include them in the checksum.
        let sum = buf
            .iter()
            .take(PACKET_LEN - 2)
            .map(|&byte| byte as u16)
            .sum();
        let checksum = u16::from_be_bytes([buf[PACKET_LEN - 2], buf[PACKET_LEN - 1]]);
        if sum != checksum {
            return Err(Error::Checksum { sum, checksum });
        }

        // bytes 0 and 1 are the magic, which we already looked at
        // bytes 2 and 3 are the length field, which i don't get why they send,
        // because the data sheet also tells us how long the packet is lol

        // now we get to the good stuff:
        let reading = Reading {
            pm1_0_standard: u16::from_be_bytes([buf[4], buf[5]]),
            pm2_5_standard: u16::from_be_bytes([buf[6], buf[7]]),
            pm10_0_standard: u16::from_be_bytes([buf[8], buf[9]]),

            pm1_0: u16::from_be_bytes([buf[9], buf[10]]),
            pm2_5: u16::from_be_bytes([buf[11], buf[12]]),
            pm10_0: u16::from_be_bytes([buf[13], buf[14]]),

            particles_0_3um: u16::from_be_bytes([buf[15], buf[16]]),
            particles_0_5um: u16::from_be_bytes([buf[17], buf[18]]),
            particles_2_5um: u16::from_be_bytes([buf[19], buf[20]]),
            particles_5_0um: u16::from_be_bytes([buf[21], buf[22]]),
            particles_10_0um: u16::from_be_bytes([buf[23], buf[24]]),
        };

        // remaining bytes are version, error code (not documented lol), and
        // the checksum, which we already looked at

        Ok(reading)
    }
}

// === impl
