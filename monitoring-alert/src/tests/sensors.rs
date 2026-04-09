/// Tests for the sensor parsing logic, using a fixture that mirrors the real
/// benchmark JSON captured on 2026-04-09 (AMD Ryzen 7 5700X + RTX 5060 system).
///
/// Expected results from the real benchmark run:
/// - 15 temperature sensors collected
/// - 2 NVMe drives: Warning Temperature + Critical Temperature entries skipped
/// - GPU detected via "images_icon/nvidia.png" in ImageURL
/// - CPU Total load = 1.1 %
/// - GPU Core load  = 0.0 %
use crate::sensors::{collect, extract_snapshot, is_gpu_node, parse_number, Node};

// ── parse_number ──────────────────────────────────────────────────────────────

#[test]
fn parse_number_french_celsius() {
    assert_eq!(parse_number("49,0 °C"), Some(49.0));
}

#[test]
fn parse_number_french_percent() {
    assert_eq!(parse_number("1,1 %"), Some(1.1));
}

#[test]
fn parse_number_integer() {
    assert_eq!(parse_number("100 °C"), Some(100.0));
}

#[test]
fn parse_number_zero() {
    assert_eq!(parse_number("0,0 %"), Some(0.0));
}

#[test]
fn parse_number_empty() {
    assert_eq!(parse_number(""), None);
}

#[test]
fn parse_number_dash() {
    // LHM reports "-" when a sensor has no value
    assert_eq!(parse_number("- °C"), None);
}

// ── is_gpu_node ───────────────────────────────────────────────────────────────

#[test]
fn gpu_nvidia_image_url() {
    assert!(is_gpu_node("images_icon/nvidia.png"));
}

#[test]
fn gpu_ati_image_url() {
    assert!(is_gpu_node("images_icon/ati.png"));
}

#[test]
fn gpu_generic_gpu_image_url() {
    assert!(is_gpu_node("images_icon/gpu.png"));
}

#[test]
fn gpu_case_insensitive() {
    assert!(is_gpu_node("images_icon/NVIDIA.PNG"));
}

#[test]
fn not_gpu_cpu_image_url() {
    assert!(!is_gpu_node("images_icon/cpu.png"));
}

#[test]
fn not_gpu_motherboard_image_url() {
    assert!(!is_gpu_node("images_icon/chip.png"));
}

// ── Temperature filtering ────────────────────────────────────────────────────

#[test]
fn warning_temperature_skipped() {
    let node = Node {
        text: "Warning Temperature".to_string(),
        value: "70,0 °C".to_string(),
        image_url: String::new(),
        sensor_type: "Temperature".to_string(),
        children: vec![],
    };
    let mut temps = vec![];
    let mut cpu = None;
    let mut gpu = None;
    collect(&node, "SSD", false, &mut temps, &mut cpu, &mut gpu);
    assert!(temps.is_empty(), "Warning Temperature must be skipped");
}

#[test]
fn critical_temperature_skipped() {
    let node = Node {
        text: "Critical Temperature".to_string(),
        value: "85,0 °C".to_string(),
        image_url: String::new(),
        sensor_type: "Temperature".to_string(),
        children: vec![],
    };
    let mut temps = vec![];
    let mut cpu = None;
    let mut gpu = None;
    collect(&node, "SSD", false, &mut temps, &mut cpu, &mut gpu);
    assert!(temps.is_empty(), "Critical Temperature must be skipped");
}

#[test]
fn zero_temperature_skipped() {
    // LHM sometimes reports 0 °C for offline/disconnected sensors — skip them.
    let node = Node {
        text: "Chipset".to_string(),
        value: "0,0 °C".to_string(),
        image_url: String::new(),
        sensor_type: "Temperature".to_string(),
        children: vec![],
    };
    let mut temps = vec![];
    let mut cpu = None;
    let mut gpu = None;
    collect(&node, "Motherboard", false, &mut temps, &mut cpu, &mut gpu);
    assert!(temps.is_empty(), "0 °C sensor must be skipped");
}

#[test]
fn over_150_temperature_skipped() {
    let node = Node {
        text: "Bad Sensor".to_string(),
        value: "200,0 °C".to_string(),
        image_url: String::new(),
        sensor_type: "Temperature".to_string(),
        children: vec![],
    };
    let mut temps = vec![];
    let mut cpu = None;
    let mut gpu = None;
    collect(&node, "HW", false, &mut temps, &mut cpu, &mut gpu);
    assert!(temps.is_empty(), "Temperatures ≥ 150 °C must be skipped");
}

#[test]
fn valid_temperature_collected() {
    let node = Node {
        text: "CPU".to_string(),
        value: "49,0 °C".to_string(),
        image_url: String::new(),
        sensor_type: "Temperature".to_string(),
        children: vec![],
    };
    let mut temps = vec![];
    let mut cpu = None;
    let mut gpu = None;
    collect(&node, "ITE IT8689E", false, &mut temps, &mut cpu, &mut gpu);
    assert_eq!(temps.len(), 1);
    assert_eq!(temps[0].sensor, "CPU");
    assert_eq!(temps[0].value, 49.0);
    assert_eq!(temps[0].hardware, "ITE IT8689E");
}

// ── Load extraction ───────────────────────────────────────────────────────────

#[test]
fn cpu_total_load_extracted() {
    let node = Node {
        text: "CPU Total".to_string(),
        value: "1,1 %".to_string(),
        image_url: String::new(),
        sensor_type: "Load".to_string(),
        children: vec![],
    };
    let mut temps = vec![];
    let mut cpu = None;
    let mut gpu = None;
    collect(&node, "AMD Ryzen 7 5700X", false, &mut temps, &mut cpu, &mut gpu);
    assert_eq!(cpu, Some(1.1));
    assert_eq!(gpu, None);
}

#[test]
fn gpu_core_load_extracted_only_for_gpu_node() {
    let node = Node {
        text: "GPU Core".to_string(),
        value: "0,0 %".to_string(),
        image_url: String::new(),
        sensor_type: "Load".to_string(),
        children: vec![],
    };
    let mut temps = vec![];
    let mut cpu = None;
    let mut gpu = None;

    // is_gpu = false → GPU Core load must be ignored
    collect(&node, "NVIDIA GeForce RTX 5060", false, &mut temps, &mut cpu, &mut gpu);
    assert_eq!(gpu, None, "GPU Core load must be ignored when is_gpu=false");

    // is_gpu = true → captured
    collect(&node, "NVIDIA GeForce RTX 5060", true, &mut temps, &mut cpu, &mut gpu);
    assert_eq!(gpu, Some(0.0));
}

#[test]
fn cpu_load_not_overwritten_by_second_occurrence() {
    let node = Node {
        text: "CPU Total".to_string(),
        value: "50,0 %".to_string(),
        image_url: String::new(),
        sensor_type: "Load".to_string(),
        children: vec![],
    };
    let mut temps = vec![];
    let mut cpu = Some(1.1); // already set
    let mut gpu = None;
    collect(&node, "CPU", false, &mut temps, &mut cpu, &mut gpu);
    assert_eq!(cpu, Some(1.1), "First cpu_load value must not be overwritten");
}

// ── Full benchmark fixture — 15 sensors ──────────────────────────────────────

/// Minimal representation of the real benchmark JSON tree.
/// Hardware layout (same as live system on 2026-04-09):
///
/// Computer
/// ├── ITE IT8689E             (motherboard chip, 5 temps)
/// ├── AMD Ryzen 7 5700X       (CPU, 2 temps + CPU Total load)
/// ├── NVIDIA GeForce RTX 5060 (GPU, ImageURL=nvidia, 2 temps + GPU Core load)
/// ├── Samsung SSD 970 EVO Plus (NVMe1, 3 temps; Warning+Critical skipped)
/// ├── CT500P310SSD8            (NVMe2, 2 temps; Warning+Critical skipped)
/// └── Samsung SSD 860 QVO      (SATA SSD, 1 temp)
///
/// Total expected temperatures: 5+2+2+3+2+1 = 15
fn benchmark_fixture() -> Node {
    fn leaf(text: &str, value: &str, sensor_type: &str) -> Node {
        Node {
            text: text.to_string(),
            value: value.to_string(),
            image_url: String::new(),
            sensor_type: sensor_type.to_string(),
            children: vec![],
        }
    }
    fn hw(text: &str, image_url: &str, children: Vec<Node>) -> Node {
        Node {
            text: text.to_string(),
            value: String::new(),
            image_url: image_url.to_string(),
            sensor_type: String::new(),
            children,
        }
    }

    let mobo = hw(
        "ITE IT8689E",
        "images_icon/chip.png",
        vec![
            leaf("System", "33,0 °C", "Temperature"),
            leaf("VSoC MOS", "42,0 °C", "Temperature"),
            leaf("CPU", "49,0 °C", "Temperature"),
            leaf("VRM MOS", "47,0 °C", "Temperature"),
            leaf("Chipset", "44,0 °C", "Temperature"),
        ],
    );

    let cpu = hw(
        "AMD Ryzen 7 5700X",
        "images_icon/cpu.png",
        vec![
            leaf("Core (Tctl/Tdie)", "52,5 °C", "Temperature"),
            leaf("CCD1 (Tdie)", "47,8 °C", "Temperature"),
            leaf("CPU Total", "1,1 %", "Load"),
        ],
    );

    let gpu = hw(
        "NVIDIA GeForce RTX 5060",
        "images_icon/nvidia.png",
        vec![
            leaf("GPU Core", "38,0 °C", "Temperature"),
            leaf("GPU Memory Junction", "40,0 °C", "Temperature"),
            leaf("GPU Core", "0,0 %", "Load"),
        ],
    );

    // NVMe 1: 3 real temps + 2 threshold entries that must be skipped
    let nvme1 = hw(
        "Samsung SSD 970 EVO Plus 500GB",
        "images_icon/hdd.png",
        vec![
            leaf("Composite", "41,9 °C", "Temperature"),
            leaf("Temperature #1", "40,0 °C", "Temperature"),
            leaf("Temperature #2", "44,0 °C", "Temperature"),
            leaf("Warning Temperature", "84,8 °C", "Temperature"),
            leaf("Critical Temperature", "84,8 °C", "Temperature"),
        ],
    );

    // NVMe 2: 2 real temps + 2 threshold entries
    let nvme2 = hw(
        "CT500P310SSD8",
        "images_icon/hdd.png",
        vec![
            leaf("Composite", "37,9 °C", "Temperature"),
            leaf("Temperature #1", "35,0 °C", "Temperature"),
            leaf("Warning Temperature", "84,8 °C", "Temperature"),
            leaf("Critical Temperature", "84,8 °C", "Temperature"),
        ],
    );

    // SATA SSD: 1 temp
    let sata = hw(
        "Samsung SSD 860 QVO 1TB",
        "images_icon/hdd.png",
        vec![leaf("Temperature", "28,0 °C", "Temperature")],
    );

    let computer = Node {
        text: "PC-JORDAN".to_string(),
        value: String::new(),
        image_url: String::new(),
        sensor_type: String::new(),
        children: vec![mobo, cpu, gpu, nvme1, nvme2, sata],
    };

    Node {
        text: "Sensor".to_string(),
        value: String::new(),
        image_url: String::new(),
        sensor_type: String::new(),
        children: vec![computer],
    }
}

#[test]
fn benchmark_total_temperature_count() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    assert_eq!(
        snap.temperatures.len(),
        15,
        "Expected 15 temperatures from benchmark fixture, got {}. Sensors: {:?}",
        snap.temperatures.len(),
        snap.temperatures
            .iter()
            .map(|t| format!("{}/{}", t.hardware, t.sensor))
            .collect::<Vec<_>>()
    );
}

#[test]
fn benchmark_warning_critical_not_in_results() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    for t in &snap.temperatures {
        assert!(
            !t.sensor.contains("Warning"),
            "Warning Temperature must not appear in results: {}/{}",
            t.hardware,
            t.sensor
        );
        assert!(
            !t.sensor.contains("Critical"),
            "Critical Temperature must not appear in results: {}/{}",
            t.hardware,
            t.sensor
        );
    }
}

#[test]
fn benchmark_cpu_load() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    assert_eq!(snap.cpu_load, Some(1.1), "CPU Total load mismatch");
}

#[test]
fn benchmark_gpu_load() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    assert_eq!(snap.gpu_load, Some(0.0), "GPU Core load mismatch");
}

#[test]
fn benchmark_mobo_sensors() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    let mobo: Vec<_> = snap
        .temperatures
        .iter()
        .filter(|t| t.hardware == "ITE IT8689E")
        .collect();
    assert_eq!(mobo.len(), 5, "Expected 5 motherboard sensors");
}

#[test]
fn benchmark_cpu_sensors() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    let cpu_temps: Vec<_> = snap
        .temperatures
        .iter()
        .filter(|t| t.hardware.contains("Ryzen"))
        .collect();
    assert_eq!(cpu_temps.len(), 2, "Expected 2 CPU temperature sensors");
}

#[test]
fn benchmark_gpu_sensors() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    let gpu_temps: Vec<_> = snap
        .temperatures
        .iter()
        .filter(|t| t.hardware.contains("RTX"))
        .collect();
    assert_eq!(gpu_temps.len(), 2, "Expected 2 GPU temperature sensors");
}

#[test]
fn benchmark_nvme1_sensors() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    let nvme1: Vec<_> = snap
        .temperatures
        .iter()
        .filter(|t| t.hardware.contains("970 EVO"))
        .collect();
    assert_eq!(
        nvme1.len(),
        3,
        "Expected 3 sensors for Samsung 970 EVO (Warning+Critical skipped)"
    );
}

#[test]
fn benchmark_nvme2_sensors() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    let nvme2: Vec<_> = snap
        .temperatures
        .iter()
        .filter(|t| t.hardware == "CT500P310SSD8")
        .collect();
    assert_eq!(
        nvme2.len(),
        2,
        "Expected 2 sensors for CT500P310SSD8 (Warning+Critical skipped)"
    );
}

#[test]
fn benchmark_sata_ssd_sensor() {
    let root = benchmark_fixture();
    let snap = extract_snapshot(&root).expect("extract_snapshot");
    let sata: Vec<_> = snap
        .temperatures
        .iter()
        .filter(|t| t.hardware.contains("860 QVO"))
        .collect();
    assert_eq!(sata.len(), 1, "Expected 1 sensor for Samsung 860 QVO");
    assert_eq!(sata[0].value, 28.0);
}
