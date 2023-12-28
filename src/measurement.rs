use heapless::{HistoryBuffer, Vec};

use crate::hardware_config as hw;
use crate::util::{LaxDuration, LaxMonotonic};

#[derive(Clone, Debug)]
pub struct Calibration {
    sum: u64,
    count: u32,
}

#[derive(Clone, Debug)]
pub enum CalibrationState {
    Done(u16),
    InProgress(Calibration),
}

impl Calibration {
    fn new() -> Self {
        Self { sum: 0, count: 0 }
    }

    pub fn add(&mut self, value: u16) {
        self.sum += value as u64;
        self.count += 1;
    }

    fn finish(&self) -> u16 {
        if self.count == 0 {
            return 0;
        }
        (self.sum / self.count as u64) as u16
    }
}

impl CalibrationState {
    pub fn begin(&mut self) {
        *self = CalibrationState::InProgress(Calibration::new());
    }

    pub fn finish(&mut self) -> u16 {
        match *self {
            CalibrationState::InProgress(ref calibration) => {
                let value = calibration.finish();
                *self = CalibrationState::Done(value);
                value
            }
            CalibrationState::Done(value) => value,
        }
    }
}

impl Default for CalibrationState {
    fn default() -> Self {
        Self::Done(0)
    }
}

const MARGIN_SAMPLES: usize = 100;
pub const RING_BUFFER_LEN: usize = 500;
pub type RingBuffer = HistoryBuffer<u16, RING_BUFFER_LEN>;

// const MARGIN_SAMPLES: usize = 10;
// pub const RING_BUFFER_LEN: usize = 1000;

#[derive(Copy, Clone)]
pub struct SampleRate {
    sample_rate: u32,
    sample_rate_counter: u32,
}

impl SampleRate {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            sample_rate_counter: 0,
        }
    }

    fn step(&mut self) -> bool {
        self.sample_rate_counter += 1;
        self.sample_rate_counter %= self.sample_rate;
        self.sample_rate_counter == 0
    }

    fn mul(&mut self, ratio: u32) {
        self.sample_rate *= ratio;
    }
}

pub struct SamplingBuffer<'a, const LEN: usize> {
    buffer: &'a mut HistoryBuffer<u16, LEN>,
    sample_rate: SampleRate,
    samples_since_start: usize,
}

pub enum SamplingBufferWriteResult {
    Discarded,
    Sampled,
    SampledAndCompacted { factor: u32 },
}

impl<'a, const LEN: usize> SamplingBuffer<'a, LEN> {
    pub fn new(buffer: &'a mut HistoryBuffer<u16, LEN>) -> Self {
        Self {
            buffer,
            sample_rate: SampleRate::new(1),
            samples_since_start: 0,
        }
    }

    pub fn into_inner(self) -> &'a mut HistoryBuffer<u16, LEN> {
        self.buffer
    }

    pub fn samples_since_start(&self) -> usize {
        self.samples_since_start
    }

    pub fn sample_rate(&self) -> &SampleRate {
        &self.sample_rate
    }

    pub fn extend_pre_buffer<I: Iterator<Item = u16>>(&mut self, iter: I) {
        self.buffer.extend(iter)
    }

    #[inline(always)]
    pub fn write(&mut self, value: u16) -> SamplingBufferWriteResult {
        if !self.sample_rate.step() {
            return SamplingBufferWriteResult::Discarded;
        }

        self.buffer.write(value);
        self.samples_since_start += 1;

        if self.buffer.len() > self.buffer.capacity() - MARGIN_SAMPLES {
            // Compactify samples in the buffer by discarding every 2nd item
            let mut new_buffer = HistoryBuffer::new();
            let mut iter = self.buffer.iter();
            while let Some(item) = iter.next() {
                new_buffer.write(*item);
                // Skip every other one
                if iter.next().is_none() {
                    break;
                }
            }
            *self.buffer = new_buffer;

            let factor = 2;
            self.samples_since_start /= factor as usize;
            self.sample_rate.mul(factor);

            return SamplingBufferWriteResult::SampledAndCompacted { factor };
        }

        return SamplingBufferWriteResult::Sampled;
    }
}

#[derive(Clone, Debug)]
pub struct MeasurementResult {
    pub duration_micros: u64,
    pub integrated_duration_micros: u64,
    pub sample_buffer: RingBuffer,
    pub samples_since_start: usize,
    pub samples_since_end: usize,
}

pub enum Measurement<'a, M: LaxMonotonic> {
    Idle {
        buffer: &'a mut RingBuffer,
        trigger_high: u16,
        trigger_low: u16,
    },
    Measuring {
        since: M::Instant,
        peak: u16,
        integrated: u64, // samples x (abs value)
        trigger_low: u16,
        sampling_buffer: SamplingBuffer<'a, RING_BUFFER_LEN>,
    },
    Trailing {
        buffer: &'a mut RingBuffer,
        samples_since_start: usize,
        samples_since_end: usize,
        duration_micros: u64,
        integrated_duration_micros: u64,
        sample_rate: SampleRate,
    },
    Done(MeasurementResult),
}

static mut TMP_BUFFER: RingBuffer = RingBuffer::new();

impl<'a, M: LaxMonotonic> Measurement<'a, M> {
    pub fn new(calibration_value: u16, buffer: &'a mut RingBuffer) -> Self {
        Self::Idle {
            buffer,
            trigger_low: (calibration_value as f32 * hw::TRIGGER_THRESHOLD_LOW) as u16,
            trigger_high: (calibration_value as f32 * hw::TRIGGER_THRESHOLD_HIGH) as u16,
        }
    }

    pub fn new_debug_duration(ms: u32) -> Self {
        Self::Done(MeasurementResult {
            duration_micros: ms as u64 * 1000,
            integrated_duration_micros: ms as u64 * 1000,
            sample_buffer: RingBuffer::new(),
            samples_since_start: 0,
            samples_since_end: 0,
        })
    }

    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done { .. })
    }

    pub fn step(&mut self, value: u16) {
        match self {
            Self::Idle {
                buffer,
                trigger_high,
                trigger_low,
            } => {
                buffer.write(value);
                if value > *trigger_high {
                    let now = M::now();

                    let buffer = core::mem::replace(buffer, unsafe { &mut TMP_BUFFER });
                    // Now look back for the trigger_low
                    let mut ordered_buffer = Vec::<_, RING_BUFFER_LEN>::new();
                    ordered_buffer.extend(buffer.oldest_ordered().skip(buffer.len()).copied());

                    buffer.clear();
                    let mut sampling_buffer = SamplingBuffer::new(buffer);

                    // let ordered_buffer = ordered_buffer
                    //     .into_iter()
                    //     .skip(last_index_above_trigger.saturating_sub(MARGIN_SAMPLES))
                    //     .collect();

                    // sampling_buffer.extend_pre_buffer(ordered_buffer.iter().copied());

                    let last_index_above_trigger = ordered_buffer
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, &x)| x < *trigger_low)
                        .map(|(i, _)| i)
                        .unwrap_or(0);

                    // let last_index_above_trigger = ordered_buffer.len() - 1;

                    // // A - include rise in integration
                    // sampling_buffer.extend_pre_buffer(ordered_buffer[last_index_above_trigger.saturating_sub(MARGIN_SAMPLES)..last_index_above_trigger].into_iter().copied());

                    // let mut integrated = 0;
                    // for sample in &ordered_buffer[last_index_above_trigger..] {
                    //     integrated += *sample as u64;
                    //     sampling_buffer.write(*sample);
                    // }
                    // // ---

                    // B - dont include rise in integration
                    sampling_buffer.extend_pre_buffer(
                        ordered_buffer[last_index_above_trigger.saturating_sub(MARGIN_SAMPLES)..]
                            .into_iter()
                            .copied(),
                    );

                    let mut integrated = 0;
                    // ---

                    // todo add these samples to integrated

                    *self = Self::Measuring {
                        since: now,
                        sampling_buffer,
                        peak: value,
                        integrated,
                        trigger_low: *trigger_low,
                    };
                }
            }
            Self::Measuring {
                since,
                sampling_buffer,
                integrated,
                peak,
                trigger_low,
            } => {
                *peak = (*peak).max(value);
                match sampling_buffer.write(value) {
                    SamplingBufferWriteResult::Discarded => (),
                    SamplingBufferWriteResult::Sampled => {
                        *integrated += value as u64;
                    }
                    SamplingBufferWriteResult::SampledAndCompacted { factor } => {
                        *integrated += value as u64;
                        *integrated /= factor as u64;
                    }
                }

                if value < *trigger_low {
                    let t_end = M::now();

                    let samples_since_start = sampling_buffer.samples_since_start();
                    let sample_rate = sampling_buffer.sample_rate().clone();

                    let sample_buffer = core::mem::replace(
                        sampling_buffer,
                        SamplingBuffer::new(unsafe { &mut TMP_BUFFER }),
                    )
                    .into_inner();

                    // remove area below threshold
                    let integrated_value_samples =
                        *integrated - samples_since_start as u64 * *trigger_low as u64;

                    // scale Y to 0-1
                    let integrated_duration_samples =
                        integrated_value_samples / (*peak - *trigger_low) as u64;

                    let duration_micros = (t_end - *since).to_micros();
                    let integrated_duration_micros =
                        integrated_duration_samples * duration_micros / samples_since_start as u64;

                    *self = Self::Trailing {
                        duration_micros,
                        buffer: sample_buffer,
                        samples_since_start: samples_since_start,
                        samples_since_end: 0,
                        integrated_duration_micros,
                        sample_rate,
                    };
                    return;
                }
            }
            Self::Trailing {
                duration_micros,
                buffer,
                samples_since_start,
                samples_since_end,
                integrated_duration_micros,
                sample_rate,
            } => {
                if !sample_rate.step() {
                    return;
                }

                let margin = MARGIN_SAMPLES / sample_rate.sample_rate as usize;

                if *samples_since_end < margin {
                    buffer.write(value);
                    *samples_since_end += 1;
                    *samples_since_start += 1;
                } else {
                    // Reduce margins for short exposures
                    let final_margin = margin.min(*samples_since_start - *samples_since_end);

                    let buffer_len = buffer.len();
                    let iter = buffer.iter();

                    let end_index = buffer_len - *samples_since_end;
                    let iter = iter.take(end_index + final_margin);

                    let start_index = buffer_len.checked_sub(*samples_since_start);
                    let mut iter = iter.skip(
                        start_index
                            .and_then(|x| x.checked_sub(final_margin))
                            .unwrap_or(0),
                    );

                    let mut final_buffer = RingBuffer::new();
                    final_buffer.extend(&mut iter);

                    *samples_since_start -= margin - final_margin;
                    *samples_since_end -= margin - final_margin;

                    *self = Self::Done(MeasurementResult {
                        duration_micros: *duration_micros,
                        integrated_duration_micros: *integrated_duration_micros,
                        sample_buffer: final_buffer,
                        samples_since_start: *samples_since_start,
                        samples_since_end: *samples_since_end,
                    });
                }
            }
            Self::Done { .. } => (),
        }
    }

    pub fn take_result(self) -> Option<MeasurementResult> {
        match self {
            Self::Done(result) => Some(result),
            _ => None,
        }
    }
}
