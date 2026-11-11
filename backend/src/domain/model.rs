//! The data the application is about.

/// One temperature reported by one sensor.
pub struct TemperatureReading {
    pub hardware: String,
    pub sensor: String,
    pub value: f64,
}

/// What one poll of the sensors returns.
pub struct SnapshotData {
    pub temperatures: Vec<TemperatureReading>,
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
}
