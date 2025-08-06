use std::{
    io::{BufRead, BufReader},
    thread::sleep,
    time::Duration,
};

use scpi_client::{
    EmptyResponse, ScpiDeserialize, ScpiRequest, ScpiSerialize, check_empty, match_literal,
};
use serialport::{SerialPort, SerialPortInfo};

use crate::commands::{
    CurrentRange, DifferentialConversionRequest, DisableRequest, EepromAddress, EnableRequest,
    EnableVoltageCalibrationModeRequest, IdentityRequest,
    LockCurrentRangeAndClearCalibrationRequest, MeasureRequest, MeasureResponse, ReadEepromRequest,
    ResetRequest, SetCurrentLimitDacRequest, SetCurrentLimitRequest, SetOverSampleRateRequest,
    SetVoltageDacRequest, SetVoltageRequest, WriteCurrentLimitCalibrationRequest,
    WriteCurrentLimitDacCalibrationRequest, WriteVoltageAdcCalibrationRequest,
    WriteVoltageDacCalibrationRequest,
};

pub type Current = uom::si::f32::ElectricCurrent;
pub type Voltage = uom::si::f32::ElectricPotential;

pub use uom::si::electric_current::{ampere, milliampere};
pub use uom::si::electric_potential::{millivolt, volt};

pub mod commands;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    ScpiClient(#[from] scpi_client::Error),
    #[error("IOError: {0}")]
    IoError(#[from] std::io::Error),
    #[error("serialport error: {0}")]
    Serialport(#[from] serialport::Error),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct MicroSmu {
    port: Box<dyn SerialPort>,
}

impl MicroSmu {
    pub fn open(port: SerialPortInfo) -> Result<MicroSmu> {
        const BAUDRATE: u32 = 9600;
        let port = serialport::new(port.port_name, BAUDRATE)
            // We need a gracious timeout because the device will not answer
            // while performing the measurement and stalls the connection.
            // The value is based on the python reference implementation.
            // Note, that for high over sampling values this is still not sufficient.
            .timeout(Duration::from_millis(1000))
            .open()?;
        let smu = Self::new(port);
        Ok(smu)
    }

    pub fn new(port: Box<dyn SerialPort>) -> MicroSmu {
        Self { port }
    }

    fn send(&mut self, request: impl ScpiSerialize) -> Result<()> {
        let mut out = String::new();
        out.reserve(32);

        request.serialize(&mut out);
        out.push('\n');

        assert!(out.is_ascii());

        self.port.write_all(out.as_bytes())?;

        // The device needs a small pause after transmission,
        // otherwise we run into IOError timeouts.
        // The value is based on the python reference implementation,
        // but smaller delays may be acceptable.
        sleep(Duration::from_millis(50));

        Ok(())
    }

    pub fn send_command<Request>(&mut self, request: Request) -> Result<()>
    where
        Request: ScpiRequest<Response = EmptyResponse>,
    {
        self.send(request)?;
        Ok(())
    }

    pub fn query<Request, Response>(&mut self, request: Request) -> Result<Response>
    where
        Request: ScpiRequest<Response = Response>,
        Response: ScpiDeserialize,
    {
        self.send(request)?;

        let mut reader = BufReader::new(&mut self.port);
        let mut data = String::new();
        reader.read_line(&mut data)?;
        let mut data = data.as_str();
        let response = Response::deserialize(&mut data)?;
        match_literal(&mut data, "\n")?;
        check_empty(data)?;
        Ok(response)
    }

    /// Enable SMU output
    pub fn enable(&mut self) -> Result<()> {
        self.send_command(EnableRequest)?;
        Ok(())
    }

    /// Disable SMU output (high impedance)
    pub fn disable(&mut self) -> Result<()> {
        self.send_command(DisableRequest)?;
        Ok(())
    }

    /// Set the sink/source current limit.
    ///
    /// `limit` is the absolute value and is applied as limit to both source and sink current,
    /// although sink induces a negative sign in the measurements.
    ///
    /// Panics, if limit is below zero or exceeds 40mA (the maximum current capability of the SMU).
    pub fn set_current_limit(&mut self, limit: Current) -> Result<()> {
        self.send_command(SetCurrentLimitRequest::new(limit))?;
        Ok(())
    }

    /// Set the SMU to the requested voltage level in volts
    pub fn set_voltage(&mut self, voltage: Voltage) -> Result<()> {
        self.send_command(SetVoltageRequest { voltage })?;
        Ok(())
    }

    /// Set the SMU to the requested voltage level and return the measured voltage and current
    pub fn measure(&mut self, voltage: Voltage) -> Result<MeasureResponse> {
        let response = self.query(MeasureRequest { voltage })?;
        Ok(response)
    }

    /// Set the oversample rate.
    ///
    /// This is the number of samples that are averaged for a given measurement
    pub fn set_over_sample_rate(&mut self, samples: u16) -> Result<()> {
        self.send_command(SetOverSampleRateRequest { samples })?;
        Ok(())
    }

    /// Set the voltage DAC to this level.
    pub fn set_voltage_dac(&mut self, level: u16) -> Result<()> {
        self.send_command(SetVoltageDacRequest { level })?;
        Ok(())
    }

    /// Perform a differential conversion between adjacent ADC channels
    /// Only channel 0 and 2 can be used for differential conversion.
    /// The differential measurement is sampled with the next adjacent channel, so 0 with 1 and 2 with 3.
    ///
    /// Panics if channel is invalid for differential conversion.
    pub fn manual_measure_differential_channel(&mut self, channel: u8) -> Result<u16> {
        let response = self.query(DifferentialConversionRequest::new(channel))?;
        Ok(response.value)
    }

    /// Set the current limit DAC to this level.
    pub fn set_current_limit_dac(&mut self, level: u16) -> Result<()> {
        self.send_command(SetCurrentLimitDacRequest { level })?;
        Ok(())
    }

    /// Enable voltage calibration mode.
    pub fn enable_voltage_calibration_mode(&mut self) -> Result<()> {
        self.send_command(EnableVoltageCalibrationModeRequest)?;
        Ok(())
    }

    /// Lock current range and temporarily clear current calibration data.
    pub fn lock_current_range_and_clear_calibration(&mut self, range: CurrentRange) -> Result<()> {
        self.send_command(LockCurrentRangeAndClearCalibrationRequest { range })?;
        Ok(())
    }

    /// Write a float to the EEPROM address of int.
    ///
    /// Always panics as unimplemented.
    /// See [commands::WriteEepromRequest].
    pub fn write_eeprom(&mut self, _address: u16, _value: f32) -> Result<()> {
        unimplemented!("Unavailable, see documentation.");
    }

    /// Read the float stored in the requested EEPROM address.
    pub fn read_eeprom(&mut self, address: EepromAddress) -> Result<f32> {
        let response = self.query(ReadEepromRequest { address })?;
        Ok(response.value)
    }

    /// Reset the uSMU. This will cause the VCP to disconnect and will require reconnecting.
    pub fn reset(mut self) -> Result<()> {
        self.send_command(ResetRequest)?;
        Ok(())
    }

    /// Read the uSMU identification
    pub fn get_identity(&mut self) -> Result<u32> {
        let response = self.query(IdentityRequest)?;
        Ok(response.uid)
    }

    /// Write the voltage DAC calibration to EEPROM.
    pub fn write_voltage_dac_calibration(&mut self, slope: f32, intercept: f32) -> Result<()> {
        self.send_command(WriteVoltageDacCalibrationRequest { slope, intercept })?;
        Ok(())
    }

    /// Write the voltage ADC calibration to EEPROM.
    pub fn write_voltage_adc_calibration(&mut self, slope: f32, intercept: f32) -> Result<()> {
        self.send_command(WriteVoltageAdcCalibrationRequest { slope, intercept })?;
        Ok(())
    }

    /// Write current ADC calibration for the given current range to EEPROM.
    pub fn write_current_limit_calibration(
        &mut self,
        range: CurrentRange,
        slope: f32,
        intercept: f32,
    ) -> Result<()> {
        self.send_command(WriteCurrentLimitCalibrationRequest {
            range,
            slope,
            intercept,
        })?;
        Ok(())
    }

    /// Write the current limit DAC calibration to EEPROM.
    pub fn write_current_limit_dac(&mut self, slope: f32, intercept: f32) -> Result<()> {
        self.send_command(WriteCurrentLimitDacCalibrationRequest { slope, intercept })?;
        Ok(())
    }
}

pub const USB_VID: u16 = 1155;
pub const USB_PID: u16 = 22336;

pub fn find_serial_ports() -> Result<Vec<SerialPortInfo>> {
    let ports = serialport::available_ports()?
        .into_iter()
        .filter(|e| match &e.port_type {
            serialport::SerialPortType::UsbPort(usb) => usb.pid == USB_PID && usb.vid == USB_VID,
            _ => false,
        })
        .collect();
    Ok(ports)
}
