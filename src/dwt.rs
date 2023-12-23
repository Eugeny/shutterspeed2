use cortex_m::peripheral::{DCB, DWT};
use cortex_m_microclock::CYCCNTClock;

pub struct DwtMonotonic<const SYSCLK_HZ: u32> {}

impl<const SYSCLK_HZ: u32> DwtMonotonic<SYSCLK_HZ> {
    pub fn new(dcb: &mut DCB, dwt: DWT) -> Self {
        CYCCNTClock::<SYSCLK_HZ>::init(dcb, dwt);
        DwtMonotonic {}
    }
}

impl<const SYSCLK_HZ: u32> rtic_monotonics::Monotonic for DwtMonotonic<SYSCLK_HZ> {
    type Instant = fugit::TimerInstantU64<SYSCLK_HZ>;
    type Duration = fugit::TimerDurationU64<SYSCLK_HZ>;

    #[inline(always)]
    fn now() -> Self::Instant {
        CYCCNTClock::now()
    }

    const ZERO: Self::Instant = Self::Instant::from_ticks(0);
    const TICK_PERIOD: Self::Duration = Self::Duration::from_ticks(1);

    fn clear_compare_flag() {
    }


    fn set_compare(instant: Self::Instant) {

    }

}
