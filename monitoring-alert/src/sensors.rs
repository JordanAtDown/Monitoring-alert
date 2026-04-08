use anyhow::Result;

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

#[cfg(windows)]
pub fn read_sensors(lhm_host: &str, lhm_port: u16) -> Result<SnapshotData> {
    use anyhow::Context;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Node {
        #[serde(rename = "Text")]
        text: String,
        #[serde(rename = "Value", default)]
        value: String,
        #[serde(rename = "ImageURL", default)]
        image_url: String,
        /// Sensor type on leaf nodes: "Temperature", "Load", "Fan", etc.
        /// Empty string on group/hardware nodes.
        #[serde(rename = "Type", default)]
        sensor_type: String,
        #[serde(rename = "Children")]
        children: Vec<Node>,
    }

    /// Parses a French-locale number string like "49,0 °C" or "1,1 %" → 49.0 / 1.1
    fn parse_number(s: &str) -> Option<f64> {
        s.split_whitespace()
            .next()?
            .replace(',', ".")
            .parse()
            .ok()
    }

    /// Detects GPU hardware nodes by their ImageURL (e.g. "images_icon/nvidia.png").
    /// Using ImageURL avoids false positives on "AMD Ryzen" CPU names.
    fn is_gpu_node(image_url: &str) -> bool {
        let u = image_url.to_lowercase();
        u.contains("nvidia") || u.contains("ati") || u.contains("gpu")
    }

    fn collect(
        node: &Node,
        hardware: &str,
        is_gpu: bool,
        temperatures: &mut Vec<TemperatureReading>,
        cpu_load: &mut Option<f64>,
        gpu_load: &mut Option<f64>,
    ) {
        match node.sensor_type.as_str() {
            "Temperature" => {
                // Skip threshold constants reported by NVMe drives
                if node.text.contains("Warning") || node.text.contains("Critical") {
                    // fall through to children (none expected, but safe)
                } else if let Some(v) = parse_number(&node.value) {
                    if v > 0.0 && v < 150.0 {
                        temperatures.push(TemperatureReading {
                            hardware: hardware.to_string(),
                            sensor: node.text.clone(),
                            value: v,
                        });
                    }
                }
            }
            "Load" => {
                if let Some(v) = parse_number(&node.value) {
                    let name = &node.text;
                    if (name == "CPU Total" || name.contains("CPU Package"))
                        && cpu_load.is_none()
                    {
                        *cpu_load = Some(v);
                    } else if is_gpu && name == "GPU Core" && gpu_load.is_none() {
                        *gpu_load = Some(v);
                    }
                }
            }
            _ => {}
        }
        for child in &node.children {
            collect(child, hardware, is_gpu, temperatures, cpu_load, gpu_load);
        }
    }

    let url = format!("http://{}:{}/data.json", lhm_host, lhm_port);
    let root: Node = ureq::get(&url)
        .call()
        .context(
            "Cannot reach LibreHardwareMonitor HTTP server. \
             Ensure LHM is running and Remote Web Server is enabled (Options → Remote Web Server).",
        )?
        .into_json()
        .context("Failed to parse LibreHardwareMonitor JSON response")?;

    // JSON tree: root ("Sensor") → computer ("PC-JORDAN") → hardware nodes (CPU, GPU, ...)
    let computer = root
        .children
        .first()
        .context("No computer node found in LHM response")?;

    let mut temperatures = Vec::new();
    let mut cpu_load: Option<f64> = None;
    let mut gpu_load: Option<f64> = None;

    for hw_node in &computer.children {
        let is_gpu = is_gpu_node(&hw_node.image_url);
        collect(
            hw_node,
            &hw_node.text,
            is_gpu,
            &mut temperatures,
            &mut cpu_load,
            &mut gpu_load,
        );
    }

    Ok(SnapshotData {
        temperatures,
        cpu_load,
        gpu_load,
    })
}

#[cfg(not(windows))]
pub fn read_sensors(_lhm_host: &str, _lhm_port: u16) -> Result<SnapshotData> {
    anyhow::bail!("Sensor reading is only supported on Windows")
}
