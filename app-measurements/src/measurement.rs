use heapless::HistoryBuffer;
use infinity_sampler::{SamplingOutcome, SamplingRate, SamplingReservoir};

use crate::util::{HistoryBufferDoubleEndedIterator, LaxDuration, LaxMonotonic};

#[derive(Clone, Debug, Copy)]
pub struct TriggerThresholds {
    pub low_ratio: f32,
    pub high_ratio: f32,
    pub low_delta: u16,
    pub high_delta: u16,
}

impl TriggerThresholds {
    pub fn trigger_low(&self, calibration_value: u16) -> u16 {
        ((calibration_value as f32 * self.low_ratio) + self.low_delta as f32) as u16
    }

    pub fn trigger_high(&self, calibration_value: u16) -> u16 {
        ((calibration_value as f32 * self.high_ratio) + self.high_delta as f32) as u16
    }
}

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
pub const SAMPLING_BUFFER_LEN: usize = 512;
pub const SAMPLING_BUFFER_LEN_WITH_MARGINS: usize = SAMPLING_BUFFER_LEN + 2 * MARGIN_SAMPLES;
pub type ResultBuffer = HistoryBuffer<u16, SAMPLING_BUFFER_LEN_WITH_MARGINS>;

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
    pub sample_buffer: ResultBuffer,
    pub samples_since_start: usize,
    pub samples_since_end: usize,
    pub sample_rate: SamplingRate,
}

pub struct Measurement<M: LaxMonotonic> {
    head_buffer: HistoryBuffer<u16, MARGIN_SAMPLES>,
    tail_buffer: HistoryBuffer<u16, MARGIN_SAMPLES>,
    sampling_buffer: SamplingReservoir<u16, SAMPLING_BUFFER_LEN>,
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
        head_buffer_samples: usize,
        samples_since_trigger: usize,
    },
    Trailing {
        head_buffer_samples: usize,
        tail_sample_rate: SamplingRate,
        samples_since_end: usize,
        duration_micros: u64,
        integrated_duration_micros: u64,
    },
    Done(MeasurementResult),
}

impl<M: LaxMonotonic> Measurement<M> {
    pub fn new(calibration_value: u16, trigger_thresholds: TriggerThresholds) -> Self {
        Self {
            head_buffer: HistoryBuffer::new(),
            tail_buffer: HistoryBuffer::new(),
            sampling_buffer: SamplingReservoir::new(),
            state: MeasurementState::Idle {
                trigger_low: trigger_thresholds
                    .trigger_low(calibration_value)
                    .max(calibration_value + 5),
                trigger_high: trigger_thresholds
                    .trigger_high(calibration_value)
                    .max(calibration_value + 10),
            },
        }
    }

    pub fn new_debug_duration(ms: u32) -> Self {
        Self {
            head_buffer: HistoryBuffer::new(),
            tail_buffer: HistoryBuffer::new(),
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

                    let last_index_below_trigger =
                        HistoryBufferDoubleEndedIterator::new(&self.head_buffer)
                            .enumerate()
                            .rev()
                            .find(|(_, &x)| x < *trigger_low)
                            .map(|(i, _)| i)
                            .unwrap_or(0);

                    let head_buf_integrated_samples =
                        self.head_buffer.len() - last_index_below_trigger;
                    let head_buf_integrated = self
                        .head_buffer
                        .oldest_ordered()
                        .skip(last_index_below_trigger)
                        .map(|&x| x as u64)
                        .sum::<u64>();

                    self.state = MeasurementState::Measuring {
                        since: now,
                        peak: value,
                        integrated: head_buf_integrated,
                        head_buffer_samples: head_buf_integrated_samples,
                        samples_since_trigger: 0,
                        trigger_low: *trigger_low,
                    };
                }
            }
            MeasurementState::Measuring {
                since,
                head_buffer_samples,
                samples_since_trigger,
                integrated,
                peak,
                trigger_low,
            } => {
                *peak = (*peak).max(value);
                match self.sampling_buffer.sample(value) {
                    SamplingOutcome::Discarded(_) => (),
                    SamplingOutcome::Consumed => {
                        *integrated += value as u64;
                        *samples_since_trigger += 1;
                    }
                    SamplingOutcome::ConsumedAndRateReduced { factor } => {
                        *integrated += value as u64;
                        *integrated /= factor as u64;
                        // head buffer will be compactified later
                        *head_buffer_samples /= factor as usize;
                        *samples_since_trigger /= factor as usize;
                    }
                }

                if value < *trigger_low {
                    let t_end = M::now();

                    // remove area below threshold
                    let integrated_value_samples =
                        *integrated - *samples_since_trigger as u64 * *trigger_low as u64;

                    // scale Y to 0-1
                    let integrated_duration_samples =
                        integrated_value_samples / (*peak - *trigger_low) as u64;

                    let duration_micros = (t_end - *since).to_micros();
                    let integrated_duration_micros =
                        integrated_duration_samples * duration_micros / *samples_since_trigger as u64;

                    self.state = MeasurementState::Trailing {
                        duration_micros,
                        tail_sample_rate: self.sampling_buffer.sampling_rate().clone(),
                        head_buffer_samples: *head_buffer_samples,
                        samples_since_end: 0,
                        integrated_duration_micros,
                    }
                }
            }
            MeasurementState::Trailing {
                duration_micros,
                tail_sample_rate,
                head_buffer_samples,
                samples_since_end,
                integrated_duration_micros,
            } => {
                if tail_sample_rate.step() {
                    self.tail_buffer.write(value);
                    *samples_since_end += 1;
                }

                let sample_rate = self.sampling_buffer.sampling_rate();

                if *samples_since_end >= MARGIN_SAMPLES {
                    let mut iter = self.sampling_buffer.ordered_iter();

                    let mut final_buffer = ResultBuffer::new();
                    final_buffer.extend(
                        self.head_buffer
                            .oldest_ordered()
                            .step_by(sample_rate.divisor() as usize),
                    );
                    final_buffer.extend(&mut iter);
                    final_buffer.extend(self.tail_buffer.oldest_ordered());

                    self.state = MeasurementState::Done(MeasurementResult {
                        duration_micros: *duration_micros,
                        integrated_duration_micros: *integrated_duration_micros,
                        samples_since_start: self.sampling_buffer.len()
                            + self.tail_buffer.len()
                            + *head_buffer_samples,
                        samples_since_end: self.tail_buffer.len(),
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
