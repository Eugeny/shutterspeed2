use cortex_m_microclock::CYCCNTClock;
use heapless::HistoryBuffer;
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

pub struct HistoryBufferDoubleEndedIterator<'a, T, const N: usize> {
    buf: &'a HistoryBuffer<T, N>,
    cur: usize,
    cur_back: usize,
}

impl<'a, T, const N: usize> HistoryBufferDoubleEndedIterator<'a, T, N> {
    pub fn new(buf: &'a HistoryBuffer<T, N>) -> Self {
        Self {
            buf,
            cur: 0,
            cur_back: buf.len(),
        }
    }
}

impl<'a, T, const N: usize> ExactSizeIterator for HistoryBufferDoubleEndedIterator<'a, T, N> {}

impl<'a, T, const N: usize> Iterator for HistoryBufferDoubleEndedIterator<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let (a, b) = self.buf.as_slices();
        self.cur += 1;
        if self.cur < a.len() {
            Some(&a[self.cur])
        } else if self.cur < a.len() + b.len() {
            Some(&b[self.cur - a.len()])
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.buf.len(), Some(self.buf.len()))
    }
}

impl<'a, T, const N: usize> DoubleEndedIterator for HistoryBufferDoubleEndedIterator<'_, T, N> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let (a, b) = self.buf.as_slices();
        if self.cur_back == 0 {
            return None;
        }
        self.cur_back -= 1;
        if self.cur_back < a.len() {
            Some(&a[self.cur_back])
        } else if self.cur_back < a.len() + b.len() {
            Some(&b[self.cur_back - a.len()])
        } else {
            None
        }
    }
}
