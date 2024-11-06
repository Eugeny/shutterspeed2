#![no_std]

mod measurement;
pub mod util;
pub use measurement::*;
#[cfg(feature = "cortex-m")]
pub use util::CycleCounterClock;
pub use infinity_sampler::SamplingRate;
