use ufmt::{uWrite, uwrite};

pub fn write_fraction<W: uWrite>(s: &mut W, fraction: f32) {
    let int = fraction as u32;
    let fr = (fraction - int as f32) * 10.0;
    let _ = uwrite!(s, "{}", int);
    if int < 10 {
        let _ = uwrite!(s, ".{}", fr as u32);
    }
}

pub fn write_micros<W: uWrite>(s: &mut W, micros: u64) {
    if micros > 10000 {
        let millis = micros / 1000;
        let _ = uwrite!(s, "{} ms", millis);
    } else {
        let _ = uwrite!(s, "{} us", micros);
    }
}
