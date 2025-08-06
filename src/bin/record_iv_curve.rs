use std::{path::PathBuf, process::ExitCode, thread::sleep, time::Duration};

use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use ndarray::linspace;
use serde::Serialize;
use uom::si::{f32::Time, time::second};
use usmu::{
    Current, MicroSmu, Result, Voltage, ampere, commands::MeasureResponse, find_serial_ports, volt,
};

#[derive(Debug, Clone, ValueEnum, Parser, PartialEq, Eq)]
enum OutputFormat {
    Csv,
}

#[derive(Debug, Parser)]
struct CommandlineArguments {
    #[arg(long)]
    port: Option<PathBuf>,

    #[arg(long, short = 'o')]
    output: Option<PathBuf>,

    #[arg(long, short = 'f', default_value = "csv")]
    format: OutputFormat,

    #[arg(long)]
    serial_number: Option<String>,

    #[arg(long, short = 's', default_value = "-1 V")]
    voltage_start: Voltage,

    #[arg(long, short = 'e', default_value = "1 V")]
    voltage_end: Voltage,

    #[arg(long, short = 'n', default_value_t = 50)]
    voltage_steps: usize,

    #[arg(long, short = 'c', default_value = "20 mA")]
    current_limit: Current,

    /// Number of samples averaged per measurement.
    #[arg(long, short = 'r', default_value_t = 10)]
    over_sampling: u16,

    /// Time delay to wait before taking a measurement.
    #[arg(long, short = 'd', default_value = "0 ms")]
    delay: Time,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args = CommandlineArguments::parse();

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

    if ports.len() > 1 && args.port.is_none() && args.serial_number.is_none() {
        eprintln!("Available devices:");
        for (port, serial) in ports.iter() {
            eprintln!("{} - {}", port.port_name, serial);
        }

        Err(anyhow!(
            "Multiple uSMUs are attached, but neitherr port nor serial number are defined. Specify at least one to disambiguate the device."
        ))?;
    }

    if let Some(port) = args.port {
        ports.retain(|(e, _)| {
            e.port_name == port.to_str().expect("path to string conversion failed")
        });
    }
    if let Some(serial_number) = args.serial_number {
        ports.retain(|(_, e)| e == serial_number.as_str());
    }

    assert_eq!(ports.len(), 1);
    let (port, _) = ports.into_iter().next().unwrap();

    let mut smu = MicroSmu::open(port)?;

    smu.enable()?;
    smu.set_current_limit(args.current_limit)?;
    smu.set_over_sample_rate(args.over_sampling)?;

    let mut samples = Vec::with_capacity(args.voltage_steps);

    for set_voltage in linspace(
        args.voltage_start.get::<volt>(),
        args.voltage_end.get::<volt>(),
        args.voltage_steps,
    ) {
        // unfortunately, we need to unpack and repack the voltage here to use the linspace iterator :'(
        let set_voltage = Voltage::new::<volt>(set_voltage);
        smu.set_voltage(set_voltage)?;
        sleep(Duration::from_secs_f32(args.delay.get::<second>()));
        let MeasureResponse { voltage, current } = smu.measure(set_voltage)?;
        samples.push((voltage, current));
    }

    smu.disable()?;

    let out: Box<dyn std::io::Write> = if let Some(output) = args.output {
        let file = std::fs::File::create(output)?;
        Box::new(file)
    } else {
        Box::new(std::io::stdout())
    };

    #[derive(Serialize)]
    struct Sample {
        voltage: f32,
        current: f32,
    }
    let mut writer = csv::WriterBuilder::new().from_writer(out);
    assert_eq!(args.format, OutputFormat::Csv);
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
