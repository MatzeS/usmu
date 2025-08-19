use std::{io::Write, path::PathBuf, thread::sleep, time::Duration};

use crate::{
    Current, MicroSmu, Result, Voltage, ampere, commands::MeasureResponse, find_serial_ports, volt,
};
use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use ndarray::linspace;
use serde::Serialize;
use uom::si::{f32::Time, time::second};

#[derive(Debug, Clone, ValueEnum, Parser, PartialEq, Eq)]
pub enum OutputFormat {
    Csv,
}

#[derive(Debug, Parser)]
pub struct CommandlineArguments {
    #[command(flatten)]
    connection_parameter: SmuConnectionParameter,

    #[command(flatten)]
    recording_parameter: IvCurveRecordingParameters,

    #[command(flatten)]
    output_parameter: OutputParameter,
}

#[derive(Debug, Clone, Parser)]
pub struct SmuConnectionParameter {
    #[arg(long)]
    port: Option<PathBuf>,
    #[arg(long)]
    serial_number: Option<String>,
}

#[derive(Debug, Clone, Parser)]
pub struct IvCurveRecordingParameters {
    #[arg(long, short = 's', default_value = "-1 V")]
    pub start_voltage: Voltage,

    #[arg(long, short = 'e', default_value = "1 V")]
    pub end_voltage: Voltage,

    #[arg(long, short = 'n', default_value_t = 50)]
    pub voltage_steps: usize,

    #[arg(long, short = 'c', default_value = "20 mA")]
    pub current_limit: Current,

    /// Number of samples averaged per measurement.
    #[arg(long, short = 'r', default_value_t = 10)]
    pub over_sampling: u16,

    /// Time delay to wait before taking a measurement.
    #[arg(long, short = 'd', default_value = "0 ms")]
    pub delay: Time,
}

#[derive(Debug, Clone, Parser)]
pub struct OutputParameter {
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    #[arg(long, short = 'f', default_value = "csv")]
    pub format: OutputFormat,
}

impl CommandlineArguments {
    pub fn run(&self) -> Result<()> {
        let mut smu = self.connection_parameter.connect()?;
        let samples = self.recording_parameter.record(&mut smu)?;
        self.output_parameter.output(samples)?;

        Ok(())
    }
}

impl SmuConnectionParameter {
    pub fn connect(&self) -> Result<MicroSmu> {
        let ports = find_serial_ports()?;

        let mut ports = ports
            .into_iter()
            .map(|port| {
                let serial = MicroSmu::open(port.clone())
                    .ok()
                    .and_then(|mut e| e.get_identity().ok())
                    .map(|e| format!("{e}"))
                    .unwrap_or("<failed to read>".to_string());

                (port, serial)
            })
            .collect::<Vec<_>>();

        if ports.is_empty() {
            Err(anyhow!(
                "Could not find uSMU. No matching serial port identified."
            ))?;
        }

        if ports.len() > 1 && self.port.is_none() && self.serial_number.is_none() {
            eprintln!("Available devices:");
            for (port, serial) in ports.iter() {
                eprintln!("{} - {}", port.port_name, serial);
            }

            Err(anyhow!(
                "Multiple uSMUs are attached, but neitherr port nor serial number are defined. Specify at least one to disambiguate the device."
            ))?;
        }

        if let Some(port) = self.port.as_ref() {
            ports.retain(|(e, _)| {
                e.port_name == port.to_str().expect("path to string conversion failed")
            });
        }
        if let Some(serial_number) = self.serial_number.as_ref() {
            ports.retain(|(_, e)| e == serial_number.as_str());
        }

        assert_eq!(ports.len(), 1);
        let (port, _) = ports.into_iter().next().unwrap();

        let smu = MicroSmu::open(port)?;

        Ok(smu)
    }
}

impl IvCurveRecordingParameters {
    pub fn record(&self, smu: &mut MicroSmu) -> Result<Vec<(Voltage, Current)>> {
        smu.set_voltage(self.start_voltage)?;
        smu.set_current_limit(self.current_limit)?;
        smu.enable()?;
        smu.set_over_sample_rate(self.over_sampling)?;

        let mut samples = Vec::with_capacity(self.voltage_steps);

        for set_voltage in linspace(
            self.start_voltage.get::<volt>(),
            self.end_voltage.get::<volt>(),
            self.voltage_steps,
        ) {
            // unfortunately, we need to unpack and repack the voltage here to use the linspace iterator :'(
            let set_voltage = Voltage::new::<volt>(set_voltage);
            smu.set_voltage(set_voltage)?;
            sleep(Duration::from_secs_f32(self.delay.get::<second>()));
            let MeasureResponse { voltage, current } = smu.measure(set_voltage)?;
            samples.push((voltage, current));
        }

        smu.disable()?;

        Ok(samples)
    }
}

impl OutputParameter {
    pub fn output(&self, samples: Vec<(Voltage, Current)>) -> Result<()> {
        match self.format {
            OutputFormat::Csv => self.write_csv(samples),
        }
    }

    fn output_writer(&self) -> Result<Box<dyn Write>> {
        if let Some(output) = self.output.as_ref() {
            let file = std::fs::File::create(output)?;
            Ok(Box::new(file))
        } else {
            Ok(Box::new(std::io::stdout()))
        }
    }

    fn write_csv(&self, samples: Vec<(Voltage, Current)>) -> Result<()> {
        #[derive(Serialize)]
        struct Sample {
            voltage: f32,
            current: f32,
        }

        let output = self.output_writer()?;
        let mut writer = csv::WriterBuilder::new().from_writer(output);
        for (v, i) in samples {
            writer
                .serialize(Sample {
                    voltage: v.get::<volt>(),
                    current: i.get::<ampere>(),
                })
                .map_err(|e| anyhow::anyhow!(e))?;
        }
        writer.flush()?;

        Ok(())
    }
}
