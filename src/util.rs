use core::ops::{Deref, DerefMut};

use cortex_m_microclock::CYCCNTClock;
use heapless::String;
use rtic_monotonics::systick::Systick;
use rtic_monotonics::Monotonic;
use ufmt::uWrite;

#[derive(Default, Debug)]
pub struct EString<const L: usize>(String<L>);

impl<const L: usize> Deref for EString<L> {
    type Target = String<L>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const L: usize> DerefMut for EString<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const L: usize> uWrite for EString<L> {
    type Error = core::fmt::Error;

    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.push_str(s).map_err(|_| core::fmt::Error)
    }

    fn write_char(&mut self, c: char) -> Result<(), Self::Error> {
        self.0.push(c).map_err(|_| core::fmt::Error)
    }
}

pub trait LaxDuration {
    fn to_micros(&self) -> u64;
}

impl LaxDuration for fugit::MicrosDurationU32 {
    fn to_micros(&self) -> u64 {
        self.to_micros() as u64
    }
}

impl LaxDuration for fugit::Duration<u32, 1, 1000> {
    fn to_micros(&self) -> u64 {
        self.to_micros() as u64
    }
}

impl<const CLK: u32> LaxDuration for fugit::TimerDurationU64<CLK> {
    fn to_micros(&self) -> u64 {
        self.to_micros()
    }
}

pub trait LaxMonotonic {
    type Instant: Ord
        + Copy
        + core::ops::Add<Self::Duration, Output = Self::Instant>
        + core::ops::Sub<Self::Duration, Output = Self::Instant>
        + core::ops::Sub<Self::Instant, Output = Self::Duration>;
    type Duration: LaxDuration;
    fn now() -> Self::Instant;
}

impl LaxMonotonic for Systick {
    type Instant = <Systick as Monotonic>::Instant;
    type Duration = <Systick as Monotonic>::Duration;

    fn now() -> Self::Instant {
        <Systick as Monotonic>::now()
    }
}

pub struct CycleCounterClock<const CLK: u32> {}

impl<const CLK: u32> LaxMonotonic for CycleCounterClock<CLK> {
    type Instant = fugit::TimerInstantU64<CLK>;
    type Duration = fugit::TimerDurationU64<CLK>;

    fn now() -> Self::Instant {
        CYCCNTClock::now()
    }
}
