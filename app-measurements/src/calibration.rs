use heapless::HistoryBuffer;
use infinity_sampler::SamplingRate;

#[derive(Clone, Debug, Copy)]
pub struct TriggerThresholds {
    pub low_ratio: f32,
    pub high_ratio: f32,
    pub low_delta: u16,
    pub high_delta: u16,
}

impl TriggerThresholds {
    pub fn trigger_low(&self, calibration: &CalibrationResult) -> u16 {
        (((calibration.max as f32 * self.low_ratio) + self.low_delta as f32) as u16)
            .max(calibration.max + 5)
    }

    pub fn trigger_high(&self, calibration: &CalibrationResult) -> u16 {
        (((calibration.max as f32 * self.high_ratio) + self.high_delta as f32) as u16)
            .max(calibration.max + 10)
    }
}

const CALIBRATION_SAMPLES: usize = 1024;
const CALIBRATION_SAMPLE_RATE_DIVISOR: u32 = 50;

#[derive(Clone, Debug, Default)]
pub struct CalibrationResult {
    pub average: u16,
    pub min: u16,
    pub max: u16,
}

#[derive(Clone)]
pub enum CalibrationState {
    Done(CalibrationResult),
    InProgress {
        buffer: HistoryBuffer<u16, CALIBRATION_SAMPLES>,
        rate: SamplingRate,
    },
}

impl CalibrationState {
    pub fn begin(&mut self) {
        *self = CalibrationState::InProgress {
            buffer: <_>::default(),
            rate: SamplingRate::new(CALIBRATION_SAMPLE_RATE_DIVISOR),
        };
    }

    pub fn step(&mut self, value: u16) {
        match *self {
            CalibrationState::InProgress {
                ref mut buffer,
                ref mut rate,
            } => {
                if rate.step() {
                    buffer.write(value);
                    if buffer.len() == buffer.capacity() {
                        let sum = buffer.iter().fold(0, |acc, &x| acc + x as u64);
                        let count = buffer.len() as u32;
                        let average = (sum / count as u64) as u16;
                        *self = CalibrationState::Done(CalibrationResult {
                            average,
                            min: *buffer.iter().min().unwrap(),
                            max: *buffer.iter().max().unwrap(),
                        });
                    }
                }
            }
            CalibrationState::Done(_) => {}
        }
    }

    pub fn progress(&self) -> Option<u8> {
        match *self {
            CalibrationState::InProgress { ref buffer, .. } => {
                Some((buffer.len() * 100 / buffer.capacity()) as u8)
            }
            CalibrationState::Done(_) => None,
        }
    }
}

impl Default for CalibrationState {
    fn default() -> Self {
        Self::Done(CalibrationResult::default())
    }
}
