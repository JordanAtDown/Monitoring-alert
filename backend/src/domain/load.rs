//! Load levels: temperatures are only comparable at an equivalent load.

/// Classifies a load percentage as `idle`, `light`, `moderate` or `heavy`.
pub fn load_category(load: f64) -> &'static str {
    match load as u32 {
        0..=14 => "idle",
        15..=39 => "light",
        40..=74 => "moderate",
        75..=100 => "heavy",
        _ => "heavy",
    }
}

/// Returns the effective load to use for categorisation.
///
/// Takes the **maximum** of CPU and GPU load so that GPU-intensive workloads
/// (e.g. gaming: GPU 90 %, CPU 15 %) are classified as `heavy` rather than
/// `light`. This ensures GPU temperatures recorded during gaming sessions are
/// compared against other gaming sessions, not against idle-CPU periods.
pub fn effective_load(cpu: Option<f64>, gpu: Option<f64>) -> Option<f64> {
    [cpu, gpu].into_iter().flatten().reduce(f64::max)
}
