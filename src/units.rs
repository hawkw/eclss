/// Given a temperature in Celcius and a relative humidity percentage, returns
/// an absolute humidity in grams/m^3.
// TODO(eliza): can we avoid some of the float math?
pub fn absolute_humidity(temp_c: f32, rel_humidity_percent: f32) -> f32 {
    // first, determine the saturation vapor pressure (`P_sat`) at `temp_c`
    // degrees --- the pressure when the relative humidity is 100%. we compute
    // this using a variant of the Magnus-Tetens formula:
    // (see https://doi.org/10.1175/1520-0493(1980)108%3C1046:TCOEPT%3E2.0.CO;2)
    let p_sat = 6.112 * ((17.64 * temp_c) / (temp_c + 243.5)).exp();
    // using `P_sat`, the pressure at 100% RH, we can compute `P`, the pressure
    // at the given relative humidity percentage, by multiplying:
    //     P = P_sat * (rel_humidity_percent / 100)
    // knowing the pressure, we then multiply `P` by the molecular weight
    // of water (18.02) to give the absolute humidity in grams/m^3.
    //
    // this calculation simplifies to:
    (p_sat * rel_humidity_percent * 2.1674) / (273.15 + temp_c)
    // see https://carnotcycle.wordpress.com/2012/08/04/how-to-convert-relative-humidity-to-absolute-humidity/
}


#[cfg(test)]
mod tests {
    const TEST_EPSILON: f32 = 0.5; // it's an approximation lol
    use super::*;
    macro_rules! assert_float_eq {
        ($a:expr, $b:expr) => {
            let a = dbg!($a);
            let b = $b;
            assert!((a - b).abs() < TEST_EPSILON, "{a} != {b} (~{TEST_EPSILON}")
        }
    }

    // shoutout to the wonderful table from Wikipedia for a giant pile of
    // test values:
    // https://en.wikipedia.org/wiki/Humidity#Relationship_between_absolute-,_relative-humidity,_and_temperature
    #[test]
    fn absolute_humidity_50_c() {
        assert_float_eq!(absolute_humidity(50.0, 0.0), 0.0);
        assert_float_eq!(absolute_humidity(50.0, 10.0), 8.3);
        assert_float_eq!(absolute_humidity(50.0, 20.0), 16.7);
        assert_float_eq!(absolute_humidity(50.0, 30.0), 24.9);
    }
}