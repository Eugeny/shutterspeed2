use heapless::HistoryBuffer;

use crate::util::{LaxMonotonic, LaxDuration};

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

pub type RingBuffer = HistoryBuffer<u16, 2000>;

#[derive(Clone, Debug)]
pub struct MeasurementResult {
    pub duration_micros: u64,
    pub rise_buffer: RingBuffer,
    pub fall_buffer: RingBuffer,
}

pub enum MeasurementState<M: LaxMonotonic> {
    Idle,
    Measuring {
        since: M::Instant,
        rise_buffer: RingBuffer,
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
            state: MeasurementState::Idle,
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
            MeasurementState::Idle => {
                if value > self.expected_high {
                    self.state = MeasurementState::Measuring {
                        since: M::now(),
                        rise_buffer: RingBuffer::new(),
                        fall_buffer: RingBuffer::new(),
                    };
                    self.sample_start = self.sample_ctr;
                }
            }
            MeasurementState::Measuring {
                ref since,
                ref mut rise_buffer,
                ref mut fall_buffer,
            } => {
                if value < self.expected_low {
                    let t_end = M::now();
                    self.sample_end = self.sample_ctr;
                    self.state = MeasurementState::Done(MeasurementResult {
                        duration_micros: (t_end - *since).to_micros(),
                        rise_buffer: rise_buffer.clone(),
                        fall_buffer: fall_buffer.clone(),
                    });
                    return;
                }
                self.sum += value as u64;
                self.max = self.max.max(value);
                self.sample_ctr += 1;

                if rise_buffer.len() < rise_buffer.capacity() {
                    rise_buffer.write(value);
                }

                fall_buffer.write(value);
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
