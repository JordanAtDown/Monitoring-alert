use anyhow::Result;
#[cfg(any(windows, test))]
use serde::Deserialize;

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

// ── Parsing helpers — used on Windows and in tests ───────────────────────────

#[cfg(any(windows, test))]
#[derive(Deserialize)]
pub(crate) struct Node {
    #[serde(rename = "Text")]
    pub text: String,
    #[serde(rename = "Value", default)]
    pub value: String,
    #[serde(rename = "ImageURL", default)]
    pub image_url: String,
    /// Sensor type on leaf nodes: "Temperature", "Load", "Fan", etc.
    /// Empty string on group/hardware nodes.
    #[serde(rename = "Type", default)]
    pub sensor_type: String,
    #[serde(rename = "Children")]
    pub children: Vec<Node>,
}

/// Parses a French-locale number string like "49,0 °C" or "1,1 %" → 49.0 / 1.1
#[cfg(any(windows, test))]
pub(crate) fn parse_number(s: &str) -> Option<f64> {
    s.split_whitespace().next()?.replace(',', ".").parse().ok()
}

/// Detects GPU hardware nodes by their ImageURL (e.g. "images_icon/nvidia.png").
/// Using ImageURL avoids false positives on "AMD Ryzen" CPU names.
#[cfg(any(windows, test))]
pub(crate) fn is_gpu_node(image_url: &str) -> bool {
    let u = image_url.to_lowercase();
    u.contains("nvidia") || u.contains("ati") || u.contains("gpu")
}

#[cfg(any(windows, test))]
pub(crate) fn collect(
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
                if (name == "CPU Total" || name.contains("CPU Package")) && cpu_load.is_none() {
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

/// Builds a `SnapshotData` from the LHM JSON tree (parsed `Node`).
#[cfg(any(windows, test))]
pub(crate) fn extract_snapshot(root: &Node) -> Result<SnapshotData> {
    use anyhow::Context;
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

// ── HTTP read — Windows only ──────────────────────────────────────────────────

#[cfg(windows)]
pub fn read_sensors(lhm_host: &str, lhm_port: u16) -> Result<SnapshotData> {
    use anyhow::Context;

    let url = format!("http://{}:{}/data.json", lhm_host, lhm_port);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(5))
        .timeout_read(std::time::Duration::from_secs(10))
        .build();
    let root: Node = agent
        .get(&url)
        .call()
        .context(
            "Cannot reach LibreHardwareMonitor HTTP server. \
             Ensure LHM is running and Remote Web Server is enabled (Options → Remote Web Server).",
        )?
        .into_json()
        .context("Failed to parse LibreHardwareMonitor JSON response")?;

    extract_snapshot(&root)
}

#[cfg(not(windows))]
pub fn read_sensors(_lhm_host: &str, _lhm_port: u16) -> Result<SnapshotData> {
    anyhow::bail!("Sensor reading is only supported on Windows")
}
