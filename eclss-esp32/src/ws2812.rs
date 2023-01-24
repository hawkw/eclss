use anyhow::Context;
use esp_idf_hal::{
    gpio,
    peripheral::Peripheral,
    rmt::{config::TransmitConfig, FixedLengthSignal, PinState, Pulse, RmtChannel, TxRmtDriver},
};
use std::time::Duration;

pub struct NeoPixel<'driver> {
    tx: TxRmtDriver<'driver>,
    one: (Pulse, Pulse),
    zero: (Pulse, Pulse),
}

impl<'driver> NeoPixel<'driver> {
    pub fn new(
        pin: impl Peripheral<P = impl gpio::OutputPin + 'static> + 'static,
        channel: impl Peripheral<P = impl RmtChannel> + 'driver,
    ) -> anyhow::Result<Self> {
        let config = TransmitConfig::new().clock_divider(1);
        let tx = TxRmtDriver::new(channel, pin, &config)
            .context("failed to initialize NeoPixel TX RMT driver")?;

        let ticks_hz = tx
            .counter_clock()
            .context("failed to get TX RMT driver counter clock")?;

        let pulse_nanos = |pin_state: PinState, nanos: u64| {
            Pulse::new_with_duration(ticks_hz, pin_state, &Duration::from_nanos(nanos))
                .with_context(|| {
                    format!("failed to construct pulse ({pin_state:?} for {nanos} ns)")
                })
        };
        let zero = (
            pulse_nanos(PinState::High, 350)?,
            pulse_nanos(PinState::Low, 800)?,
        );
        let one = (
            pulse_nanos(PinState::High, 700)?,
            pulse_nanos(PinState::Low, 600)?,
        );

        Ok(Self { tx, zero, one })
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) -> anyhow::Result<&mut Self> {
        let rgb = u32::from_le_bytes([g, r, b, 0]);
        const SIGNAL_LEN: usize = 24;
        let mut signal = FixedLengthSignal::<SIGNAL_LEN>::new();
        for i in 0..SIGNAL_LEN {
            let bit = rgb & (1 << i);
            let bit = if bit != 0 { self.one } else { self.zero };
            signal.set(i, &bit).context("failed to set signal bit")?;
        }

        self.tx
            .start_blocking(&signal)
            .context("failed to send blocking RMT signal")?;
        Ok(self)
    }
}
