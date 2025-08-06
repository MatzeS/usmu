use scpi_client::{
    EmptyResponse, Result, ScpiDeserialize, ScpiSerialize, impl_scpi_request, impl_scpi_serialize,
    match_literal,
};
use uom::si::electric_current::{ampere, milliampere};

use crate::{Current, Voltage, volt};

struct FormatVolt(Voltage);

impl ScpiSerialize for FormatVolt {
    fn serialize(&self, out: &mut String) {
        let value = self.0.get::<volt>();
        let encoded = format!("{}", value);
        out.push_str(encoded.as_str());
    }
}

impl From<Voltage> for FormatVolt {
    fn from(value: Voltage) -> Self {
        Self(value)
    }
}

struct FormatMilliAmpere(Current);

impl ScpiSerialize for FormatMilliAmpere {
    fn serialize(&self, out: &mut String) {
        let value = self.0.get::<milliampere>();
        let encoded = format!("{}", value);
        out.push_str(encoded.as_str());
    }
}

impl From<Current> for FormatMilliAmpere {
    fn from(value: Current) -> Self {
        Self(value)
    }
}

pub struct EnableRequest;
impl_scpi_serialize!(EnableRequest, ["CH1:ENA"]);
impl_scpi_request!(EnableRequest, EmptyResponse);

pub struct DisableRequest;
impl_scpi_serialize!(DisableRequest, ["CH1:DIS"]);
impl_scpi_request!(DisableRequest, EmptyResponse);

pub struct SetCurrentLimitRequest {
    limit: Current,
}
impl SetCurrentLimitRequest {
    /// Set the sink/source current limit.
    ///
    /// `limit` is the absolute value and is applied as limit to both source and sink current,
    /// although sink induces a negative sign in the measurements.
    ///
    /// Panics, if limit is below zero or exceeds 40mA (the maximum current capability of the SMU).
    pub fn new(limit: Current) -> Self {
        assert!(limit.is_sign_positive());
        assert!(limit.get::<milliampere>() <= 40.0);
        Self { limit }
    }
}
impl_scpi_serialize!(
    SetCurrentLimitRequest,
    ["CH1:CUR ", limit as FormatMilliAmpere]
);
impl_scpi_request!(SetCurrentLimitRequest, EmptyResponse);

pub struct SetVoltageRequest {
    pub voltage: Voltage,
}
impl_scpi_serialize!(SetVoltageRequest, ["CH1:VOL ", voltage as FormatVolt]);
impl_scpi_request!(SetVoltageRequest, EmptyResponse);

pub struct MeasureRequest {
    pub voltage: Voltage,
}
impl_scpi_serialize!(MeasureRequest, ["CH1:MEA:VOL ", voltage as FormatVolt]);

pub struct MeasureResponse {
    pub voltage: Voltage,
    pub current: Current,
}
impl ScpiDeserialize for MeasureResponse {
    fn deserialize(input: &mut &str) -> Result<Self> {
        let voltage = f32::deserialize(input)?;
        let voltage = Voltage::new::<volt>(voltage);
        match_literal(input, ",")?;
        let current = f32::deserialize(input)?;
        let current = Current::new::<ampere>(current);
        Ok(Self { voltage, current })
    }
}
impl_scpi_request!(MeasureRequest, MeasureResponse);

pub struct SetOverSampleRateRequest {
    pub samples: u16,
}
impl_scpi_serialize!(SetOverSampleRateRequest, ["CH1:OSR ", samples]);
impl_scpi_request!(SetOverSampleRateRequest, EmptyResponse);

pub struct SetVoltageDacRequest {
    pub level: u16,
}
impl_scpi_serialize!(SetVoltageDacRequest, ["DAC ", level]);
impl_scpi_request!(SetVoltageDacRequest, EmptyResponse);

pub struct DifferentialConversionRequest {
    channel: u8,
}
impl_scpi_serialize!(DifferentialConversionRequest, ["ADC ", channel]);

impl DifferentialConversionRequest {
    pub fn new(channel: u8) -> Self {
        assert!(
            channel == 0 || channel == 2,
            "Differential measurements can only be performed on channel zero or channel two."
        );
        Self { channel }
    }

    pub fn channel_zero() -> Self {
        Self::new(0)
    }

    pub fn channel_two() -> Self {
        Self::new(2)
    }
}

pub struct DifferentialConversionResponse {
    pub value: u16,
}

impl ScpiDeserialize for DifferentialConversionResponse {
    fn deserialize(input: &mut &str) -> Result<Self> {
        let value = u16::deserialize(input)?;
        Ok(Self { value })
    }
}

impl_scpi_request!(
    DifferentialConversionRequest,
    DifferentialConversionResponse
);

pub struct SetCurrentLimitDacRequest {
    pub level: u16,
}
impl SetCurrentLimitDacRequest {
    /// Panics if the value exceeds the 12 least significant bits.
    pub fn new(level: u16) -> Self {
        assert!(level >> 12 == 0);
        Self { level }
    }
}
impl_scpi_serialize!(SetCurrentLimitDacRequest, ["ILIM ", level]);
impl_scpi_request!(SetCurrentLimitDacRequest, EmptyResponse);

pub struct EnableVoltageCalibrationModeRequest;
impl_scpi_serialize!(EnableVoltageCalibrationModeRequest, ["CH1:VCAL"]);
impl_scpi_request!(EnableVoltageCalibrationModeRequest, EmptyResponse);

pub struct LockCurrentRangeAndClearCalibrationRequest {
    pub range: CurrentRange,
}
impl_scpi_serialize!(
    LockCurrentRangeAndClearCalibrationRequest,
    ["CH1:RANGE", range]
);
impl_scpi_request!(LockCurrentRangeAndClearCalibrationRequest, EmptyResponse);

#[derive(Debug, Clone, Copy)]
pub struct EepromAddress {
    pub value: u8,
}
impl_scpi_serialize!(EepromAddress, [value]);

/// Looking into the [firmware implementation][firmware],
/// this command looks to be not correctly implemented on the SMU side.
/// Hence, I would consider it highly experimental. Even if the firmware is eventually
/// fixed the firmware version must be verified before using the command.
///
/// This serialization implementation is based on the [command documentation][doc],
/// which disagrees with the firmware.
///
/// Consider using the other write commands to change the calibration.
///
/// [firmware]: https://github.com/joeltroughton/uSMU/blob/3fdb82477a9f5ed1c374189c9d4eb9d7cdb289f6/Firmware/For%20HW%20version%2010/Core/Src/main.c#L727
/// [doc]: https://github.com/joeltroughton/uSMU/tree/main/Firmware/For%20HW%20version%2010
pub struct WriteEepromRequest {
    pub address: EepromAddress,
    pub value: f32,
}
impl_scpi_serialize!(WriteEepromRequest, ["WRITE ", address, " ", value]);
impl_scpi_request!(WriteEepromRequest, EmptyResponse);

pub struct ReadEepromRequest {
    pub address: EepromAddress,
}
impl_scpi_serialize!(ReadEepromRequest, ["*READ ", address]);

pub struct ReadEepromResponse {
    pub value: f32,
}
impl ScpiDeserialize for ReadEepromResponse {
    fn deserialize(input: &mut &str) -> Result<Self> {
        let value = f32::deserialize(input)?;
        Ok(Self { value })
    }
}
impl_scpi_request!(ReadEepromRequest, ReadEepromResponse);

pub struct ResetRequest;
impl_scpi_serialize!(ResetRequest, ["*RST"]);
impl_scpi_request!(ResetRequest, EmptyResponse);

pub struct IdentityRequest;
impl_scpi_serialize!(IdentityRequest, ["*IDN?"]);

pub struct IdentityResponse {
    pub uid: u32,
}
impl ScpiDeserialize for IdentityResponse {
    fn deserialize(input: &mut &str) -> Result<Self> {
        match_literal(input, "uSMU version 1.0 ID:")?;
        let uid = u32::deserialize(input)?;
        Ok(IdentityResponse { uid })
    }
}
impl_scpi_request!(IdentityRequest, IdentityResponse);

pub struct WriteVoltageDacCalibrationRequest {
    pub slope: f32,
    pub intercept: f32,
}
impl_scpi_serialize!(
    WriteVoltageDacCalibrationRequest,
    ["CAL:DAC ", slope, " ", intercept]
);
impl_scpi_request!(WriteVoltageDacCalibrationRequest, EmptyResponse);

pub struct WriteVoltageAdcCalibrationRequest {
    pub slope: f32,
    pub intercept: f32,
}
impl_scpi_serialize!(
    WriteVoltageAdcCalibrationRequest,
    ["CAL:VOL ", slope, " ", intercept]
);
impl_scpi_request!(WriteVoltageAdcCalibrationRequest, EmptyResponse);

pub struct CurrentRange {
    value: u8,
}
impl_scpi_serialize!(CurrentRange, [value]);

impl CurrentRange {
    /// Panics, if `value` is not a valid current range (1, 2, 3 or 4).
    pub fn new(value: u8) -> Self {
        assert!(
            (1..=4).contains(&value),
            "Invalid current range '{value}', only 1 - 4 are valid."
        );
        Self { value }
    }
}

pub struct WriteCurrentLimitCalibrationRequest {
    pub range: CurrentRange,
    pub slope: f32,
    pub intercept: f32,
}
impl_scpi_serialize!(
    WriteCurrentLimitCalibrationRequest,
    ["CAL:CUR:RANGE ", range, " ", slope, " ", intercept]
);
impl_scpi_request!(WriteCurrentLimitCalibrationRequest, EmptyResponse);

pub struct WriteCurrentLimitDacCalibrationRequest {
    pub slope: f32,
    pub intercept: f32,
}
impl_scpi_serialize!(
    WriteCurrentLimitDacCalibrationRequest,
    ["CAL:ILIM ", slope, " ", intercept]
);
impl_scpi_request!(WriteCurrentLimitDacCalibrationRequest, EmptyResponse);

#[cfg(test)]
mod tests {
    use scpi_client::{ScpiDeserialize, ScpiSerialize, check_empty};

    use crate::{
        Current,
        commands::{SetCurrentLimitDacRequest, SetCurrentLimitRequest},
        milliampere,
    };

    #[test]
    fn float_conversion_is_reasonably_lossless() {
        // The finest measurement resolution of the uSMU is 10 nanoamps.
        // Hence, it is sufficient to verify these values are serialized without loss of precision.
        let mut buffer = String::new();
        let one_nano_amp = 0.000000001f32;
        one_nano_amp.serialize(&mut buffer);
        let mut data = buffer.as_str();
        let decoded = f32::deserialize(&mut data).unwrap();
        check_empty(data).unwrap();
        assert_eq!(decoded, one_nano_amp);
    }

    #[test]
    #[should_panic]
    fn current_limit_panics_for_values_below_zero() {
        SetCurrentLimitRequest::new(Current::new::<milliampere>(-1.0));
    }

    #[test]
    #[should_panic]
    fn current_limit_panics_for_excessive_values() {
        SetCurrentLimitRequest::new(Current::new::<milliampere>(100.0));
    }

    #[test]
    #[should_panic]
    fn current_limit_dac_cannot_exceed_12_bit() {
        SetCurrentLimitDacRequest::new(0b0001_0000_0000_0000);
    }
}
