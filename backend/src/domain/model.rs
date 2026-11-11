//! The data the application is about.

/// One temperature reported by one sensor.
pub struct TemperatureReading {
    pub hardware: String,
    pub sensor: String,
    pub value: f64,
}

/// A measurement that is not a temperature but explains one: how fast a fan turns, how much power
/// a processor draws. A temperature alone mixes the room, the effort and the cooling; these are
/// what let the cooling be told apart from the other two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExtraKind {
    /// Rotation speed, in RPM.
    Fan,
    /// Power drawn by a package (processor, graphics card), in watts.
    Power,
}

impl ExtraKind {
    /// How it is stored.
    pub fn as_str(self) -> &'static str {
        match self {
            ExtraKind::Fan => "fan",
            ExtraKind::Power => "power",
        }
    }

    pub fn parse(text: &str) -> Option<ExtraKind> {
        match text {
            "fan" => Some(ExtraKind::Fan),
            "power" => Some(ExtraKind::Power),
            _ => None,
        }
    }
}

/// One fan speed or power draw reported by one sensor.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtraReading {
    pub hardware: String,
    pub sensor: String,
    pub kind: ExtraKind,
    pub value: f64,
}

/// What one poll of the sensors returns.
pub struct SnapshotData {
    pub temperatures: Vec<TemperatureReading>,
    /// Fan speeds and package power draws, recorded beside the temperatures.
    pub extras: Vec<ExtraReading>,
    pub cpu_load: Option<f64>,
    pub gpu_load: Option<f64>,
}

/// A snapshot ready to be stored: the readings plus the load context they were taken in.
pub struct NewSnapshot<'a> {
    /// Local time, `YYYY-MM-DDTHH:MM:SS`.
    pub ts: &'a str,
    pub cpu_load: Option<f64>,
    pub gpu_load: Option<f64>,
    pub load_cat: &'a str,
    pub readings: &'a [TemperatureReading],
    pub extras: &'a [ExtraReading],
}
