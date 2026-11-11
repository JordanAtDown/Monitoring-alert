//! What the machine is made of, as a person names it: Processeur, Carte graphique, Carte mère,
//! Boîtier, Disque 1, Disque 2…
//!
//! LibreHardwareMonitor reports sensors per piece of *hardware*; the motherboard chip also reports
//! the processor probe and the probe inside the case. Each sensor is therefore placed on its own,
//! by what its name says. Names and sensors come from the machine: nothing here is trusted to be
//! well formed.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Cpu,
    Gpu,
    Board,
    Case,
    Disk,
    Other,
}

/// Display order of the kinds.
const KIND_ORDER: [Kind; 6] = [
    Kind::Cpu,
    Kind::Gpu,
    Kind::Board,
    Kind::Case,
    Kind::Disk,
    Kind::Other,
];

impl Kind {
    fn title(self) -> &'static str {
        match self {
            Kind::Cpu => "Processeur",
            Kind::Gpu => "Carte graphique",
            Kind::Board => "Carte mère",
            Kind::Case => "Boîtier",
            Kind::Disk => "Disque",
            Kind::Other => "Autre composant",
        }
    }

    /// A PC can have several of these: each piece of hardware is its own component, numbered.
    fn numbered(self) -> bool {
        matches!(self, Kind::Gpu | Kind::Disk)
    }
}

/// One component of the PC with the sensors that belong to it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Component {
    /// Stable identifier, usable in an address.
    pub key: String,
    pub kind: Kind,
    /// "Processeur", "Disque 2"…
    pub title: String,
    /// The model, when it says something ("AMD Ryzen 7 5700X").
    pub subtitle: Option<String>,
    /// Its sensors, as (hardware, sensor).
    pub sensors: Vec<(String, String)>,
    /// The sensor that stands for the component.
    pub main: (String, String),
}

impl Component {
    /// The same component reduced to the sensors that satisfy `keep` (those that have figures);
    /// `None` when none is left. The sensor standing for it is chosen again among them.
    pub fn restricted_to(&self, keep: impl Fn(&(String, String)) -> bool) -> Option<Component> {
        let sensors: Vec<(String, String)> =
            self.sensors.iter().filter(|s| keep(s)).cloned().collect();
        if sensors.is_empty() {
            return None;
        }
        Some(Component {
            main: pick_main(self.kind, &sensors),
            sensors,
            ..self.clone()
        })
    }
}

// ── Classification ───────────────────────────────────────────────────────────

fn starts_with_ci(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.is_char_boundary(prefix.len())
        && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn contains_ci(text: &str, needle: &str) -> bool {
    text.to_lowercase().contains(needle)
}

/// Words of a name, lowercased ("NVIDIA GeForce RTX 5060" → nvidia, geforce, rtx, 5060).
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// What a whole piece of hardware is, from its sensors first and its name when they do not say.
pub fn hardware_kind(hardware: &str, sensors: &[&str]) -> Kind {
    let any = |f: &dyn Fn(&str) -> bool| sensors.iter().any(|s| f(s));
    if any(&|s| {
        ["VRM", "VSoC", "Chipset", "System", "Motherboard", "PCH"]
            .iter()
            .any(|p| starts_with_ci(s, p))
    }) {
        return Kind::Board;
    }
    if any(&|s| starts_with_ci(s, "GPU ")) {
        return Kind::Gpu;
    }
    if any(&|s| contains_ci(s, "composite temperature")) {
        return Kind::Disk;
    }
    if any(&|s| {
        contains_ci(s, "tdie")
            || contains_ci(s, "tctl")
            || contains_ci(s, "cpu package")
            || starts_with_ci(s, "core #")
    }) {
        return Kind::Cpu;
    }
    let name = hardware.to_lowercase();
    let w = words(hardware);
    let has_word = |x: &str| w.iter().any(|y| y == x);
    if ["nvidia", "geforce", "radeon"]
        .iter()
        .any(|n| name.contains(n))
        || has_word("rtx")
        || has_word("gtx")
    {
        return Kind::Gpu;
    }
    let drive_model = w
        .iter()
        .any(|x| x.starts_with("ct") && x[2..].starts_with(|c: char| c.is_ascii_digit()));
    if [
        "ssd", "nvme", "hdd", "samsung", "seagate", "kingston", "crucial",
    ]
    .iter()
    .any(|n| name.contains(n))
        || has_word("wd")
        || drive_model
    {
        return Kind::Disk;
    }
    if [
        "gigabyte",
        "aorus",
        "asus",
        "asrock",
        "biostar",
        "supermicro",
    ]
    .iter()
    .any(|n| name.contains(n))
        || has_word("msi")
    {
        return Kind::Board;
    }
    Kind::Other
}

/// The motherboard chip also reports the processor probe and the probe inside the case.
fn sensor_kind(owner: Kind, sensor: &str) -> Kind {
    if owner != Kind::Board {
        return owner;
    }
    if starts_with_ci(sensor, "CPU") {
        Kind::Cpu
    } else if ["System", "Case", "Chassis", "Ambient"]
        .iter()
        .any(|p| starts_with_ci(sensor, p))
    {
        Kind::Case
    } else {
        Kind::Board
    }
}

/// The sensor that stands for a component: the most telling one of its kind, else the first by name.
fn pick_main(kind: Kind, sensors: &[(String, String)]) -> (String, String) {
    let preferences: &[&dyn Fn(&str) -> bool] = match kind {
        Kind::Cpu => &[
            &|s| starts_with_ci(s, "Core (Tctl") || starts_with_ci(s, "CPU Package"),
            &|s| contains_ci(s, "tctl") || contains_ci(s, "tdie"),
        ],
        Kind::Gpu => &[&|s| starts_with_ci(s, "GPU Core")],
        Kind::Board => &[&|s| starts_with_ci(s, "Chipset")],
        Kind::Case => &[&|s| starts_with_ci(s, "System")],
        Kind::Disk => &[&|s| starts_with_ci(s, "Composite Temperature"), &|s| {
            s == "Temperature"
        }],
        Kind::Other => &[],
    };
    for wanted in preferences {
        if let Some(hit) = sensors.iter().find(|(_, s)| wanted(s)) {
            return hit.clone();
        }
    }
    sensors[0].clone()
}

// ── Building the list ────────────────────────────────────────────────────────

/// Groups sensors, given as (hardware, sensor), into the components of the PC. Pass **every**
/// sensor the machine reports, even those with no figures at the moment: the numbering of the
/// disks must not change when one of them is missing from a result.
pub fn components(sensors: &[(String, String)]) -> Vec<Component> {
    let unique: BTreeSet<&(String, String)> = sensors.iter().collect();
    let mut of_hardware: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (hardware, sensor) in &unique {
        of_hardware.entry(hardware).or_default().push(sensor);
    }
    let owner: BTreeMap<&str, Kind> = of_hardware
        .iter()
        .map(|(hw, s)| (*hw, hardware_kind(hw, s)))
        .collect();

    // Numbering of the hardware of a numbered kind, alphabetical by model.
    let mut number: BTreeMap<&str, usize> = BTreeMap::new();
    let mut count: BTreeMap<usize, usize> = BTreeMap::new();
    for kind in KIND_ORDER.iter().filter(|k| k.numbered()) {
        let mut models: Vec<&str> = owner
            .iter()
            .filter(|(_, k)| *k == kind)
            .map(|(hw, _)| *hw)
            .collect();
        models.sort_by_key(|m| m.trim());
        for (i, m) in models.iter().enumerate() {
            number.insert(m, i + 1);
        }
        count.insert(*kind as usize, models.len());
    }

    struct Group {
        kind: Kind,
        title: String,
        order: usize,
        models: BTreeSet<String>,
        sensors: Vec<(String, String)>,
    }
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for (hardware, sensor) in unique {
        let own = owner[hardware.as_str()];
        let kind = sensor_kind(own, sensor);
        let key = if kind.numbered() || kind == Kind::Other {
            format!("{}:{}", kind_key(kind), hardware.trim())
        } else {
            kind_key(kind).to_string()
        };
        let group = groups.entry(key).or_insert_with(|| {
            let mut title = kind.title().to_string();
            if kind.numbered() && count.get(&(kind as usize)).copied().unwrap_or(0) > 1 {
                title = format!(
                    "{title} {}",
                    number.get(hardware.as_str()).copied().unwrap_or(0)
                );
            }
            if kind == Kind::Other {
                title = hardware.trim().to_string();
            }
            Group {
                kind,
                title,
                order: number.get(hardware.as_str()).copied().unwrap_or(0),
                models: BTreeSet::new(),
                sensors: Vec::new(),
            }
        });
        // The model is shown only when the sensor really belongs to that hardware (not the board's CPU probe).
        if kind == own && kind != Kind::Other {
            group.models.insert(hardware.trim().to_string());
        }
        group.sensors.push((hardware.clone(), sensor.clone()));
    }

    let mut out: Vec<(usize, usize, Component)> = groups
        .into_iter()
        .map(|(key, g)| {
            let main = pick_main(g.kind, &g.sensors);
            let component = Component {
                key,
                kind: g.kind,
                title: g.title,
                subtitle: (!g.models.is_empty())
                    .then(|| g.models.into_iter().collect::<Vec<_>>().join(", ")),
                sensors: g.sensors,
                main,
            };
            (g.kind as usize, g.order, component)
        })
        .collect();
    out.sort_by(|a, b| (a.0, a.1, &a.2.title).cmp(&(b.0, b.1, &b.2.title)));
    out.into_iter().map(|(_, _, c)| c).collect()
}

fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Cpu => "cpu",
        Kind::Gpu => "gpu",
        Kind::Board => "board",
        Kind::Case => "case",
        Kind::Disk => "disk",
        Kind::Other => "other",
    }
}

// ── Sensor names ─────────────────────────────────────────────────────────────

/// A readable name for a sensor, and its technical name when they differ (for those who know it).
pub fn sensor_name(sensor: &str, kind: Kind) -> (String, Option<String>) {
    if kind == Kind::Disk {
        if sensor == "Composite Temperature" || sensor == "Temperature" {
            return ("Température du disque".into(), Some(sensor.into()));
        }
        if let Some(n) = sensor
            .strip_prefix("Temperature #")
            .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        {
            return (format!("Sonde interne n° {n}"), Some(sensor.into()));
        }
    }
    let friendly = match sensor {
        "Core (Tctl/Tdie)" => "Cœurs du processeur",
        "CPU Package" => "Processeur (total)",
        "CCD1 (Tdie)" => "Puce de calcul du processeur",
        "CPU" => "Sonde de la carte mère, près du processeur",
        "GPU Core" => "Cœur de la carte graphique",
        "GPU Memory Junction" => "Mémoire de la carte graphique",
        "GPU Hot Spot" => "Point chaud de la carte graphique",
        "Chipset" => "Chipset de la carte mère",
        "VRM MOS" => "Alimentation du processeur",
        "VSoC MOS" => "Alimentation du processeur (SoC)",
        "System" => "Intérieur du boîtier",
        other => return (other.to_string(), None),
    };
    (friendly.to_string(), Some(sensor.to_string()))
}
