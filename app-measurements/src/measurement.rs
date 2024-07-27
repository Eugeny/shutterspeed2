use heapless::HistoryBuffer;
use infinity_sampler::{SamplingOutcome, SamplingRate, SamplingReservoir};

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

#[derive(Clone)]
pub struct MeasurementResult {
    pub duration_micros: u64,
    pub integrated_duration_micros: u64,
    pub sample_buffer: RingBuffer,
    pub samples_since_start: usize,
    pub samples_since_end: usize,
    pub sample_rate: SamplingRate,
}

pub struct Measurement<M: LaxMonotonic> {
    head_buffer: HistoryBuffer<u16, MARGIN_SAMPLES>,
    sampling_buffer: SamplingReservoir<u16, RING_BUFFER_LEN>,
    state: MeasurementState<M>,
}

#[allow(clippy::large_enum_variant)]
pub enum MeasurementState<M: LaxMonotonic> {
    Idle {
        trigger_high: u16,
        trigger_low: u16,
    },
    Measuring {
        since: M::Instant,
        peak: u16,
        integrated: u64, // samples x (abs value)
        trigger_low: u16,
        samples_since_start: usize,
    },
    Trailing {
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
        Self {
            head_buffer: HistoryBuffer::new(),
            sampling_buffer: SamplingReservoir::new(),
            state: MeasurementState::Idle {
                trigger_low: ((calibration_value as f32 * trigger_threshold_low) as u16)
                    .max(calibration_value + 5),
                trigger_high: ((calibration_value as f32 * trigger_threshold_high) as u16)
                    .max(calibration_value + 10),
            },
        }
    }

    pub fn new_debug_duration(ms: u32) -> Self {
        Self {
            head_buffer: HistoryBuffer::new(),
            sampling_buffer: SamplingReservoir::new(),
            state: MeasurementState::Done(MeasurementResult {
                sample_buffer: HistoryBuffer::new(),
                duration_micros: ms as u64 * 1000,
                integrated_duration_micros: ms as u64 * 1000,
                samples_since_start: 0,
                samples_since_end: 0,
                sample_rate: SamplingRate::new(1),
            }),
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, MeasurementState::Done { .. })
    }

    pub fn step(&mut self, value: u16) {
        match &mut self.state {
            MeasurementState::Idle {
                trigger_high,
                trigger_low,
            } => {
                self.head_buffer.write(value);

                if value > *trigger_high {
                    let now = M::now();

                    let last_index_above_trigger =
                        HistoryBufferDoubleEndedIterator::new(&self.head_buffer)
                            .enumerate()
                            .rev()
                            .find(|(_, &x)| x < *trigger_low)
                            .map(|(i, _)| i)
                            .unwrap_or(0);

                    // Immediately integrate any samples since then
                    // TODO maybe move this calculation to the end for perf
                    let integrated_samples = self.head_buffer.len() - last_index_above_trigger;
                    let integrated = self
                        .head_buffer
                        .oldest_ordered()
                        .skip(last_index_above_trigger)
                        .map(|&x| x as u64)
                        .sum();

                    self.state = MeasurementState::Measuring {
                        since: now,
                        peak: value,
                        integrated,
                        samples_since_start: integrated_samples,
                        trigger_low: *trigger_low,
                    };
                }
            }
            MeasurementState::Measuring {
                since,
                samples_since_start,
                integrated,
                peak,
                trigger_low,
            } => {
                *peak = (*peak).max(value);
                match self.sampling_buffer.sample(value) {
                    SamplingOutcome::Discarded(_) => (),
                    SamplingOutcome::Consumed => {
                        *integrated += value as u64;
                        *samples_since_start += 1;
                    }
                    SamplingOutcome::ConsumedAndRateReduced { factor } => {
                        *integrated += value as u64;
                        *integrated /= factor as u64;
                        *samples_since_start /= factor as usize;
                    }
                }

                if value < *trigger_low {
                    let t_end = M::now();

                    // remove area below threshold
                    let integrated_value_samples =
                        *integrated - *samples_since_start as u64 * *trigger_low as u64;

                    // scale Y to 0-1
                    let integrated_duration_samples =
                        integrated_value_samples / (*peak - *trigger_low) as u64;

                    let duration_micros = (t_end - *since).to_micros();
                    let integrated_duration_micros =
                        integrated_duration_samples * duration_micros / *samples_since_start as u64;

                    self.state = MeasurementState::Trailing {
                        duration_micros,
                        samples_since_start: *samples_since_start,
                        samples_since_end: 0,
                        integrated_duration_micros,
                    }
                }
            }
            MeasurementState::Trailing {
                duration_micros,
                samples_since_start,
                samples_since_end,
                integrated_duration_micros,
            } => {
                let outcome = self.sampling_buffer.sample(value);
                match outcome {
                    SamplingOutcome::Discarded(_) => {
                        return;
                    }
                    SamplingOutcome::ConsumedAndRateReduced { factor } => {
                        *samples_since_start /= factor as usize;
                        *samples_since_end /= factor as usize;
                    }
                    _ => (),
                }

                let sample_rate = self.sampling_buffer.sampling_rate();

                let margin = MARGIN_SAMPLES / sample_rate.divisor() as usize;

                if *samples_since_end < margin {
                    *samples_since_end += 1;
                    *samples_since_start += 1;
                } else {
                    // Reduce margins for short exposures
                    let final_margin = margin.min(*samples_since_start - *samples_since_end);

                    let buffer_len = self.sampling_buffer.len();
                    let iter = self.sampling_buffer.clone().into_ordered_iter();

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
                        self.head_buffer
                            .oldest_ordered()
                            .step_by(sample_rate.divisor() as usize),
                    );
                    final_buffer.extend(&mut iter);

                    *samples_since_start -= margin - final_margin;
                    *samples_since_end -= margin - final_margin;

                    self.state = MeasurementState::Done(MeasurementResult {
                        duration_micros: *duration_micros,
                        integrated_duration_micros: *integrated_duration_micros,
                        samples_since_start: *samples_since_start,
                        samples_since_end: *samples_since_end,
                        sample_buffer: final_buffer,
                        sample_rate: sample_rate.clone(),
                    });
                }
            }
            MeasurementState::Done { .. } => (),
        }
    }

    pub fn take_result(self) -> Option<MeasurementResult> {
        match self.state {
            MeasurementState::Done(result) => Some(result),
            _ => None,
        }
    }

    pub fn result(&self) -> Option<&MeasurementResult> {
        match &self.state {
            MeasurementState::Done(result) => Some(result),
            _ => None,
        }
    }
}
