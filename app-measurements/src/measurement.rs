use heapless::HistoryBuffer;
use infinity_sampler::{RawReservoir, SamplingOutcome, SamplingRate, SamplingReservoir};

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
pub const RING_BUFFER_LEN: usize = 512;
pub type RingBuffer = HistoryBuffer<u16, RING_BUFFER_LEN>;

#[derive(Clone)]
pub struct SamplingBuffer<const LEN: usize> {
    buffer: SamplingReservoir<u16, LEN>,
    samples_since_start: usize,
}

pub enum SamplingBufferWriteResult {
    Discarded,
    Sampled,
    SampledAndCompacted { factor: u32 },
}

impl<const LEN: usize> SamplingBuffer<LEN> {
    pub fn new(buffer: SamplingReservoir<u16, LEN>, samples_since_start: usize) -> Self {
        Self {
            buffer,
            samples_since_start,
        }
    }

    pub fn into_inner(self) -> SamplingReservoir<u16, LEN> {
        self.buffer
    }

    pub fn samples_since_start(&self) -> usize {
        self.samples_since_start
    }

    pub fn sample_rate(&self) -> &SamplingRate {
        self.buffer.sampling_rate()
    }

    #[inline(always)]
    pub fn write(&mut self, value: u16) -> SamplingOutcome<u16> {
        let outcome = self.buffer.sample(value);

        match outcome {
            SamplingOutcome::Consumed => {
                self.samples_since_start += 1;
            }
            SamplingOutcome::ConsumedAndRateReduced { factor } => {
                // Compactify samples in the buffer by discarding every 2nd item
                self.samples_since_start /= factor as usize;
            }
            _ => (),
        }
        outcome
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
pub enum Measurement<M: LaxMonotonic> {
    Idle {
        buffer: HistoryBuffer<u16, MARGIN_SAMPLES>,
        trigger_high: u16,
        trigger_low: u16,
    },
    Measuring {
        since: M::Instant,
        peak: u16,
        integrated: u64, // samples x (abs value)
        trigger_low: u16,
        head_buffer: HistoryBuffer<u16, MARGIN_SAMPLES>,
        sampling_buffer: SamplingBuffer<RING_BUFFER_LEN>,
    },
    Trailing {
        head_buffer: HistoryBuffer<u16, MARGIN_SAMPLES>,
        buffer: SamplingReservoir<u16, RING_BUFFER_LEN>,
        samples_since_start: usize,
        samples_since_end: usize,
        duration_micros: u64,
        integrated_duration_micros: u64,
    },
    Done(MeasurementResult),
}

impl<M: LaxMonotonic> Measurement<M> {
    pub fn new(
        calibration_value: u16,
        trigger_threshold_low: f32,
        trigger_threshold_high: f32,
    ) -> Self {
        Self::Idle {
            buffer: HistoryBuffer::new(),
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

                    let new_buffer = SamplingReservoir::new();

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

                    let sampling_buffer = SamplingBuffer::new(new_buffer, integrated_samples);

                    *self = Self::Measuring {
                        since: now,
                        head_buffer: buffer.clone(),
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
                head_buffer,
                sampling_buffer,
                integrated,
                peak,
                trigger_low,
            } => {
                *peak = (*peak).max(value);
                match sampling_buffer.write(value) {
                    SamplingOutcome::Discarded(_) => (),
                    SamplingOutcome::Consumed => {
                        *integrated += value as u64;
                    }
                    SamplingOutcome::ConsumedAndRateReduced { factor } => {
                        *integrated += value as u64;
                        *integrated /= factor as u64;
                    }
                }

                if value < *trigger_low {
                    let t_end = M::now();

                    let samples_since_start = sampling_buffer.samples_since_start();

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
                        head_buffer: head_buffer.clone(),
                        buffer: sampling_buffer.clone().into_inner(),
                        samples_since_start,
                        samples_since_end: 0,
                        integrated_duration_micros,
                    }
                }
            }
            Self::Trailing {
                duration_micros,
                head_buffer,
                buffer,
                samples_since_start,
                samples_since_end,
                integrated_duration_micros,
            } => {
                let outcome = buffer.sample(value);
                if matches!(outcome, SamplingOutcome::Discarded(_)) {
                    return;
                }

                let sample_rate = buffer.sampling_rate();

                let margin = MARGIN_SAMPLES / sample_rate.divisor() as usize;

                if *samples_since_end < margin {
                    *samples_since_end += 1;
                    *samples_since_start += 1;
                } else {
                    // Reduce margins for short exposures
                    let final_margin = margin.min(*samples_since_start - *samples_since_end);

                    let buffer_len = buffer.len();
                    let iter = buffer.clone().into_ordered_iter();

                    let end_index = buffer_len - *samples_since_end;
                    let iter = iter.take(end_index + final_margin);

                    let start_index = buffer_len.checked_sub(*samples_since_start);
                    let mut iter = iter.skip(
                        start_index
                            .and_then(|x| x.checked_sub(final_margin))
                            .unwrap_or(0),
                    );

                    let mut final_buffer = RingBuffer::new();
                    final_buffer.extend(
                        head_buffer
                            .oldest_ordered()
                            .step_by(sample_rate.divisor() as usize),
                    );
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
