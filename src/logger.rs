use log::{LevelFilter, Log, Metadata, Record};

pub fn init() -> Result<(), log::SetLoggerError> {
    static LOGGER: Logger = {
        #[cfg(debug_assertions)]
        let max_level = LevelFilter::Debug;
        #[cfg(not(debug_assertions))]
        let max_level = LevelFilter::Info;
        Logger { max_level }
    };

    log::set_logger(&LOGGER)?;
    log::set_max_level(LOGGER.max_level);
    Ok(())
}

#[derive(Debug)]
struct Logger {
    max_level: LevelFilter,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.max_level
    }

    fn log(&self, record: &Record<'_>) {
        static LEVEL_STRS: [&str; 5] = ["[x]", "[!]", "[i]", "[?]", "[.]"];
        let level = LEVEL_STRS[record.level() as usize - 1];
        println!("{level} {}: {}", record.target(), record.args());
    }

    fn flush(&self) {}
}
