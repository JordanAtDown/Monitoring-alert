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
        #[serde(rename = "Value")]
        value: String,
        #[serde(rename = "ImageURL")]
        image_url: String,
        #[serde(rename = "Children")]
        children: Vec<Node>,
    }

    fn parse_number(s: &str) -> Option<f64> {
        s.split_whitespace()
            .next()?
            .replace(',', ".")
            .parse()
            .ok()
    }

    fn is_gpu(text: &str) -> bool {
        let t = text.to_uppercase();
        t.contains("GPU") || t.contains("NVIDIA") || t.contains("AMD") || t.contains("ATI")
    }

    fn collect(
        node: &Node,
        hardware: &str,
        temperatures: &mut Vec<TemperatureReading>,
        cpu_load: &mut Option<f64>,
        gpu_load: &mut Option<f64>,
    ) {
        let url = node.image_url.as_str();
        if url.contains("temperature") {
            if let Some(v) = parse_number(&node.value) {
                if v > 0.0 && v < 150.0 {
                    temperatures.push(TemperatureReading {
                        hardware: hardware.to_string(),
                        sensor: node.text.clone(),
                        value: v,
                    });
                }
            }
        } else if url.contains("load") {
            if let Some(v) = parse_number(&node.value) {
                let name = &node.text;
                if (name.contains("CPU Total") || name.contains("CPU Package"))
                    && cpu_load.is_none()
                {
                    *cpu_load = Some(v);
                } else if is_gpu(hardware) && name.contains("GPU Core") && gpu_load.is_none() {
                    *gpu_load = Some(v);
                }
            }
        }
        for child in &node.children {
            collect(child, hardware, temperatures, cpu_load, gpu_load);
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

    let mut temperatures = Vec::new();
    let mut cpu_load: Option<f64> = None;
    let mut gpu_load: Option<f64> = None;

    for hw_node in &root.children {
        collect(
            hw_node,
            &hw_node.text,
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
