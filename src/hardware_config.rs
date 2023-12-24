use stm32f4xx_hal::adc::config::SampleTime;

pub const DISPLAY_BRIGHTNESS: f32 = 1.0;
pub const SAMPLE_TIME: SampleTime = SampleTime::Cycles_3;
pub const SAMPLE_RATE_HZ: u32 = 50_000_u32;
pub const SYSCLK: u32 = 84_000_000;
pub const HCLK: u32 = 42_000_000;

pub const IPRIO_ADC_TIMER: u8 = 5;

// TIM2 <-> ADC1
// TIM4 -> backlight PWM
// TIM3 -> monotonic
