use hal::adc::config::{AdcConfig, Resolution, SampleTime};
use hal::pac::ADC1;
use stm32f4xx_hal as hal;

const SAMPLE_TIME: SampleTime = SampleTime::Cycles_112;

pub struct Adc<PIN>
where
    PIN: embedded_hal::adc::Channel<ADC1>,
{
    adc: hal::adc::Adc<ADC1>,
    pin: PIN,
}

impl<PIN> Adc<PIN>
where
    PIN: embedded_hal::adc::Channel<ADC1>,
{
    pub fn new(peripheral: ADC1, pin: PIN) -> Self {
        let mut adc = hal::adc::Adc::adc1(
            peripheral,
            true,
            AdcConfig::default()
                // .dma(Dma::Continuous)
                .resolution(Resolution::Twelve)
                .default_sample_time(SAMPLE_TIME)
                .continuous(hal::adc::config::Continuous::Continuous),
        );
        // adc.configure_channel(&adc_pin, Sequence::One, SAMPLE_TIME);
        adc.start_conversion();

        Self { adc, pin }
    }
}
