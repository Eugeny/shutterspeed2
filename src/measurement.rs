use heapless::HistoryBuffer;

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

pub type RingPreBuffer = HistoryBuffer<u16, 1000>;
pub type RingBuffer = HistoryBuffer<u16, 5000>;

#[derive(Clone, Debug)]
pub struct MeasurementResult {
    pub duration_micros: u64,
    // pub rise_buffer: RingBuffer,
    pub fall_buffer: RingBuffer,
    pub samples_since_start: u32,
    pub samples_since_end: u32,
}

pub enum MeasurementState<M: LaxMonotonic> {
    Idle {
        pre_buffer: RingPreBuffer,
    },
    Measuring {
        since: M::Instant,
        // rise_buffer: RingBuffer,
        fall_buffer: RingBuffer,
        samples_since_start: u32,
    },
    Trailing {
        samples_since_start: u32,
        samples_since_end: u32,
        duration_micros: u64,
        fall_buffer: RingBuffer,
    },
    Done(MeasurementResult),
}

pub struct Measurement<M: LaxMonotonic> {
    state: MeasurementState<M>,

    max: u16,
    sample_ctr: u32,
    sum: u64,

    sample_start: u32,
    sample_end: u32,

    expected_high: u16,
    expected_low: u16,
}

impl<M: LaxMonotonic> Measurement<M> {
    pub fn new(calibration_value: u16) -> Self {
        let threshold_low = 1.1;
        let threshold_high = 2.0;
        Self {
            state: MeasurementState::Idle {
                pre_buffer: RingPreBuffer::new(),
            },
            max: 0,
            sample_ctr: 0,
            sum: 0,
            sample_end: 0,
            sample_start: 0,
            expected_low: (calibration_value as f32 * threshold_low) as u16,
            expected_high: (calibration_value as f32 * threshold_high) as u16,
        }
    }

    pub fn is_done(&self) -> bool {
        match self.state {
            MeasurementState::Done { .. } => true,
            _ => false,
        }
    }

    pub fn step(&mut self, value: u16) {
        match &mut self.state {
            MeasurementState::Idle { pre_buffer } => {
                pre_buffer.write(value);
                if value > self.expected_high {
                    let mut fall_buffer = RingBuffer::new();
                    fall_buffer.extend_from_slice(pre_buffer.as_slice());
                    self.state = MeasurementState::Measuring {
                        since: M::now(),
                        // rise_buffer: RingBuffer::new(),
                        fall_buffer,
                        samples_since_start: 0,
                    };
                    self.sample_start = self.sample_ctr;
                }
            }
            MeasurementState::Measuring {
                ref since,
                // ref mut rise_buffer,
                ref mut fall_buffer,
                ref mut samples_since_start,
            } => {
                if value < self.expected_low {
                    let t_end = M::now();
                    self.sample_end = self.sample_ctr;
                    self.state = MeasurementState::Trailing {
                        duration_micros: (t_end - *since).to_micros(),
                        fall_buffer: fall_buffer.clone(),
                        samples_since_start: *samples_since_start,
                        samples_since_end: 0,
                    };
                    return;
                }
                self.sum += value as u64;
                self.max = self.max.max(value);
                self.sample_ctr += 1;

                *samples_since_start += 1;

                // if rise_buffer.len() < rise_buffer.capacity() {
                //     rise_buffer.write(value);
                // }

                fall_buffer.write(value);
            }
            MeasurementState::Trailing {
                duration_micros,
                fall_buffer,
                ref mut samples_since_start,
                ref mut samples_since_end,
            } => {
                if *samples_since_end < 200 {
                    fall_buffer.write(value);
                    *samples_since_end += 1;
                    *samples_since_start += 1;
                } else {
                    self.state = MeasurementState::Done(MeasurementResult {
                        duration_micros: *duration_micros,
                        fall_buffer: fall_buffer.clone(),
                        samples_since_start: *samples_since_start,
                        samples_since_end: *samples_since_end,
                    });
                    return;
                }
            }
            MeasurementState::Done { .. } => (),
        }
    }

    pub fn result(&self) -> Option<&MeasurementResult> {
        match self.state {
            MeasurementState::Done(ref result) => Some(result),
            _ => None,
        }
    }

    pub fn result_samples(&self) -> u32 {
        match self.state {
            MeasurementState::Done(_) => self.sample_end - self.sample_start,
            _ => 0,
        }
    }
}
