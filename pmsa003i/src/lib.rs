// stolen from
// https://github.com/adafruit/Adafruit_PM25AQI/blob/master/Adafruit_PM25AQI.cpp
// except the bugs, which are my own :)
// and also the datasheet, which is extremely translated:
// https://cdn-shop.adafruit.com/product-files/4632/4505_PMSA003I_series_data_manual_English_V2.6.pdf
use embedded_hal::blocking::i2c;

pub struct Pmsa003i<I> {
    i2c: I,
}

#[derive(Copy, Clone, Debug)]
pub struct Reading {
    // pm1: f32,
    // pm2_5: f32,
    // pm10: f32,
    // uint16_t framelen;       ///< How long this data chunk is
    /// PM1.0 concentration in µg/𝑚3.
    pm1_0_standard: u16,
    /// PM2.5 concentration in µg/𝑚3.
    pm2_5_standard: u16,
    /// PM10.0 concentration in µg/𝑚3.
    pm10_0_standard: u16,

    /// PM1.0 concentration in µg/𝑚3, under atmospheric environment.
    ///
    /// Note: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet...
    pm1_0_env: u16,
    /// PM2.5 concentration in µg/𝑚3, under atmospheric environment.
    ///
    /// Note: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet...
    pm2_5_env: u16,
    /// PM10.0 concentration in µg/𝑚3, under atmospheric environment.
    ///
    /// Note: I don't actually know what "under atmospheric environment" means
    /// but it says that in the datasheet...
    pm10_0_env: u16,

    /// Number of particles with diameter >= 0.3 µm in 0.1L of air.
    particles_0_3um: u16,
    /// Number of particles with diameter >= 0.5 µm in 0.1L of air.
    particles_0_5um: u16,
    /// Number of particles with diameter >= 2.5 µm in 0.1L of air.
    particles_2_5um: u16,
    /// Number of particles with diameter >= 5.0 µm in 0.1L of air.
    particles_5_0um: u16,
    /// Number of particles with diameter >= 10.0 µm in 0.1L of air.
    particles_5_0um: u16,
}

pub enum Error<E> {
    I2c(E),
    Checksum { sum: u16, checksum: u16 },
    NoMagic,
}

impl<I, E> Pmsa003i<I>
where
    I: i2c::Read<Error = E> + i2c::Write<Error = E>,
{
    pub fn read(&mut self) -> Result<Reading, Error<E>> {
        const MAGIC: u16 = 0x424d;
        const PACKET_LEN: usize = 32;
        const I2C_ADDR: u8 = 0x12;

        let mut buf = [0; PACKET_LEN];
        self.i2c.read(I2C_ADDR, &mut buf[..]).map_err(Error::I2c)?;

        if u16::from_be_bytes([buf[0], buf[1]]) != MAGIC {
            // magic isn't real :(
            return Err(NoMagic);
        }

        // last two bytes are the checksum so dont include them in the checksum.
        let sum = buf
            .iter()
            .take(PACKET_LEN - 2)
            .map(|byte| byte as u16)
            .sum();
        let checksum = u16::from_be_bytes([buf[PACKET_LEN - 1], buf[PACKET_LEN]]);
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

            pm1_0_env: u16::from_be_bytes([buf[9], buf[10]]),
            pm2_5_env: u16::from_be_bytes([buf[11], buf[12]]),
            pm10_0_env: u16::from_be_bytes([buf[13], buf[14]]),

            particles_0_3um: u16::from_be_bytes([buf[15], buf[16]]),
            particles_0_5um: u16::from_be_bytes([buf[17], buf[18]]),
            particles_2_5um: u16::from_be_bytes([buf[19], buf[20]]),
            particles_5_0um: u16::from_be_bytes([buf[21], buf[22]]),
            // remaining bytes are version, error code (not documented lol), and
            // the checksum
        };
        Ok(reading)
    }
}
