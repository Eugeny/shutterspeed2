use heapless::HistoryBuffer;

use crate::util::{HistoryBufferDoubleEndedIterator, LaxDuration, LaxMonotonic};

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
    pub fn new(buffer: &'a mut HistoryBuffer<u16, LEN>, samples_since_start: usize) -> Self {
        Self {
            buffer,
            sample_rate: SampleRate::new(1),
            samples_since_start,
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
            let factor = 2;
            compactify_history_buffer(self.buffer, factor);
            self.samples_since_start /= factor;
            self.sample_rate.mul(factor as u32);

            return SamplingBufferWriteResult::SampledAndCompacted {
                factor: factor as u32,
            };
        }

        SamplingBufferWriteResult::Sampled
    }
}

fn compactify_history_buffer<T: Copy, const LEN: usize>(
    buffer: &mut HistoryBuffer<T, LEN>,
    factor: usize,
) {
    let mut new_buffer = HistoryBuffer::<T, LEN>::new();
    let mut iter = buffer.oldest_ordered();
    while let Some(item) = iter.next() {
        new_buffer.write(*item);
        // Skip every other one
        for _ in 0..factor - 1 {
            if iter.next().is_none() {
                break;
            }
        }
    }
    *buffer = new_buffer
}

#[derive(Clone, Debug)]
pub struct MeasurementResult {
    pub duration_micros: u64,
    pub integrated_duration_micros: u64,
    pub sample_buffer: RingBuffer,
    pub samples_since_start: usize,
    pub samples_since_end: usize,
}

#[allow(clippy::large_enum_variant)]
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
    pub fn new(
        calibration_value: u16,
        buffer: &'a mut RingBuffer,
        trigger_threshold_low: f32,
        trigger_threshold_high: f32,
    ) -> Self {
        Self::Idle {
            buffer,
            trigger_low: ((calibration_value as f32 * trigger_threshold_low) as u16)
                .max(calibration_value + 5),
            trigger_high: ((calibration_value as f32 * trigger_threshold_high) as u16)
                .max(calibration_value + 10),
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

                    // "Take" the buffer reference
                    let buffer = core::mem::replace(buffer, unsafe { &mut TMP_BUFFER });

                    // Now look back for the trigger_low
                    let last_index_above_trigger = HistoryBufferDoubleEndedIterator::new(buffer)
                        .enumerate()
                        .rev()
                        .find(|(_, &x)| x < *trigger_low)
                        .map(|(i, _)| i)
                        .unwrap_or(0);

                    // Immediately integrated any samples since then
                    let integrated_samples = buffer.len() - last_index_above_trigger;
                    let integrated = buffer
                        .oldest_ordered()
                        .skip(last_index_above_trigger)
                        .map(|&x| x as u64)
                        .sum();

                    let sampling_buffer = SamplingBuffer::new(buffer, integrated_samples);

                    *self = Self::Measuring {
                        since: now,
                        sampling_buffer,
                        peak: value,
                        integrated,
                        trigger_low: *trigger_low,
                    };
                    return;
                }

                if buffer.len() > MARGIN_SAMPLES * 2 {
                    // Keep the buffer small to avoid frequent resizing once
                    // sampling starts
                    compactify_history_buffer(buffer, 2);
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
                    let sample_rate = *sampling_buffer.sample_rate();

                    let sample_buffer = core::mem::replace(
                        sampling_buffer,
                        SamplingBuffer::new(unsafe { &mut TMP_BUFFER }, 0),
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
                        samples_since_start,
                        samples_since_end: 0,
                        integrated_duration_micros,
                        sample_rate,
                    }
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
