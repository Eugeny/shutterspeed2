use core::fmt::Debug;

use ufmt::{uWrite, uwrite};

pub fn write_fraction<E: Debug, W: uWrite<Error=E>>(s: &mut W, fraction: f32) {
    let int = fraction as u32;
    let fr = (fraction - int as f32) * 10.0;
    uwrite!(s, "{}", int).unwrap();
    if int < 10 {
        uwrite!(s, ".{}", fr as u32).unwrap();
    }
}

pub fn write_micros<E: Debug, W: uWrite<Error=E>>(s: &mut W, micros: u64) {
    if micros > 10000 {
        let millis = micros / 1000;
        uwrite!(s, "{} ms", millis).unwrap();
    } else {
        uwrite!(s, "{} us", micros).unwrap();
    }
}
