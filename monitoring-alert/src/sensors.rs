use anyhow::{Context, Result};

pub struct TemperatureReading {
    pub hardware: String,
    pub sensor: String,
    pub value: f64,
}

pub struct SnapshotData {
    pub temperatures: Vec<TemperatureReading>,
    pub cpu_load: Option<f64>,
    pub gpu_load: Option<f64>,
}

fn extract_hardware(parent: &str) -> String {
    parent
        .split('/')
        .find(|s| !s.is_empty())
        .unwrap_or("unknown")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

fn is_gpu_hardware(hardware_type: &str) -> bool {
    let hw = hardware_type.to_uppercase();
    hw.contains("GPU") || hw.contains("ATI") || hw.contains("NVIDIA") || hw.contains("AMD")
}

#[cfg(windows)]
pub fn read_sensors() -> Result<SnapshotData> {
    use serde::Deserialize;
    use wmi::{COMLibrary, WMIConnection};

    #[derive(Deserialize, Debug)]
    struct WmiSensor {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "Value")]
        value: f32,
        #[serde(rename = "SensorType")]
        sensor_type: String,
        #[serde(rename = "Parent")]
        parent: String,
    }

    let com_lib = COMLibrary::new().context("Failed to initialize COM library")?;
    let wmi_con =
        WMIConnection::with_namespace_path("ROOT\\LibreHardwareMonitor", com_lib)
            .context("Failed to connect to LibreHardwareMonitor WMI namespace. Ensure LibreHardwareMonitor is running with WMI support enabled.")?;

    let sensors: Vec<WmiSensor> = wmi_con
        .raw_query("SELECT Name, Value, SensorType, Parent FROM Sensor")
        .context("Failed to query WMI sensors")?;

    let mut temperatures = Vec::new();
    let mut cpu_load: Option<f64> = None;
    let mut gpu_load: Option<f64> = None;

    for s in sensors {
        let value = s.value as f64;
        if !(value > 0.0 && value < 150.0) {
            continue;
        }
        let hardware = extract_hardware(&s.parent);
        match s.sensor_type.as_str() {
            "Temperature" => {
                temperatures.push(TemperatureReading {
                    hardware,
                    sensor: s.name,
                    value,
                });
            }
            "Load" => {
                if s.name.contains("CPU Total") || s.name.contains("CPU Package") {
                    cpu_load = Some(value);
                } else if is_gpu_hardware(&hardware) && s.name.contains("GPU Core") {
                    gpu_load = Some(value);
                }
            }
            _ => {}
        }
    }

    Ok(SnapshotData {
        temperatures,
        cpu_load,
        gpu_load,
    })
}

#[cfg(not(windows))]
pub fn read_sensors() -> Result<SnapshotData> {
    anyhow::bail!("WMI sensor reading is only supported on Windows")
}
