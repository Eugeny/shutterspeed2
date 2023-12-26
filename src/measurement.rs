use heapless::HistoryBuffer;

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

pub type RingPreBuffer = HistoryBuffer<u16, 1000>;
pub type RingBuffer = HistoryBuffer<u16, 1000>;

const MARGIN_SAMPLES: usize = 200;

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

    fn halve(&mut self) {
        self.sample_rate *= 2;
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

pub enum Measurement<M: LaxMonotonic> {
    Idle {
        pre_buffer: RingPreBuffer,
        trigger_high: u16,
        trigger_low: u16,
    },
    Measuring {
        since: M::Instant,
        sample_buffer: RingBuffer,
        samples_since_start: usize,
        peak: u16,
        integrated: u64, // samples x (abs value)
        sample_rate: SampleRate,
        trigger_low: u16,
    },
    Trailing {
        samples_since_start: usize,
        samples_since_end: usize,
        duration_micros: u64,
        integrated_duration_micros: u64,
        sample_buffer: RingBuffer,
        sample_rate: SampleRate,
    },
    Done(MeasurementResult),
}

impl<M: LaxMonotonic> Default for Measurement<M> {
    fn default() -> Self {
        Self::new(0)
    }
}

impl<M: LaxMonotonic> Measurement<M> {
    pub fn new(calibration_value: u16) -> Self {
        Self::Idle {
            pre_buffer: RingPreBuffer::new(),
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
                pre_buffer,
                trigger_high,
                trigger_low,
            } => {
                pre_buffer.write(value);
                if value > *trigger_high {
                    let mut sample_buffer = RingBuffer::new();
                    sample_buffer.extend(
                        pre_buffer
                            .oldest_ordered()
                            .skip(pre_buffer.len() - MARGIN_SAMPLES),
                    );

                    *self = Self::Measuring {
                        since: M::now(),
                        sample_buffer,
                        peak: value,
                        samples_since_start: 0,
                        integrated: 0,
                        sample_rate: SampleRate::new(1),
                        trigger_low: *trigger_low,
                    };
                }
            }
            Self::Measuring {
                since,
                sample_buffer,
                samples_since_start,
                integrated,
                peak,
                sample_rate,
                trigger_low,
            } => {
                if !sample_rate.step() {
                    return;
                }

                if *samples_since_start + MARGIN_SAMPLES > sample_buffer.capacity() - MARGIN_SAMPLES
                {
                    // Compactify samples in the buffer by discarding every 2nd item
                    let mut new_buffer = RingBuffer::new();
                    let mut iter = sample_buffer.iter();
                    while let Some(item) = iter.next() {
                        new_buffer.write(*item);
                        // Skip every other one
                        if iter.next().is_none() {
                            break;
                        }
                    }
                    *sample_buffer = new_buffer;
                    *samples_since_start /= 2;
                    *integrated /= 2;
                    sample_rate.halve();
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

                    *self = Self::Trailing {
                        duration_micros,
                        sample_buffer: sample_buffer.clone(),
                        samples_since_start: *samples_since_start,
                        samples_since_end: 0,
                        integrated_duration_micros,
                        sample_rate: *sample_rate,
                    };
                    return;
                }

                *samples_since_start += 1;
                *integrated += value as u64;
                *peak = (*peak).max(value);

                sample_buffer.write(value);
            }
            Self::Trailing {
                duration_micros,
                sample_buffer,
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
                    sample_buffer.write(value);
                    *samples_since_end += 1;
                    *samples_since_start += 1;
                } else {
                    // Reduce margins for short exposures
                    let final_margin = margin.min(*samples_since_start - *samples_since_end);

                    let buffer_len = sample_buffer.len();
                    let iter = sample_buffer.oldest_ordered();

                    let end_index = buffer_len - *samples_since_end;
                    let iter = iter.take(end_index + final_margin);

                    let start_index = buffer_len.checked_sub(*samples_since_start);
                    let iter = iter.skip(
                        start_index
                            .and_then(|x| x.checked_sub(final_margin))
                            .unwrap_or(0),
                    );

                    let mut final_buffer = RingBuffer::new();
                    final_buffer.extend(iter);

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
