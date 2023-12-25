use cortex_m_microclock::CYCCNTClock;
use rtic_monotonics::systick::Systick;
use rtic_monotonics::Monotonic;

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
