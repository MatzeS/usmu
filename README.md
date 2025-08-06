# μSMU Rust Library

Straightforward implementation of the μSMU SCPI-like interface in rust.

Checkout the [IV curve recording binary](src/bin/record_iv_curve.rs) as an example.

## Notes
The following commands are manually tested: `CH1:ENA, CH1:DIS, CH1:CUR, CH1:VOL, CH1:MEA:VOL, CH1:OSR, *RST, *IDN?`.
Everything else is not tested, specifically the commands `DAC` and `ADC`, and everything regarding calibration and writing the calibration EEPROM are not tested.

