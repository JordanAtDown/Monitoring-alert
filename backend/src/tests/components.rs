use crate::domain::components::{components, hardware_kind, sensor_name, Component, Kind};

/// The sensors of a real machine: a Ryzen, an RTX, a Gigabyte board and three disks.
fn real_machine() -> Vec<(String, String)> {
    [
        ("AMD Ryzen 7 5700X", "Core (Tctl/Tdie)"),
        ("AMD Ryzen 7 5700X", "CCD1 (Tdie)"),
        ("NVIDIA GeForce RTX 5060", "GPU Core"),
        ("NVIDIA GeForce RTX 5060", "GPU Memory Junction"),
        ("Gigabyte B550I AORUS PRO AX", "System"),
        ("Gigabyte B550I AORUS PRO AX", "Chipset"),
        ("Gigabyte B550I AORUS PRO AX", "VRM MOS"),
        ("Gigabyte B550I AORUS PRO AX", "VSoC MOS"),
        ("Gigabyte B550I AORUS PRO AX", "CPU"),
        ("Samsung SSD 970 EVO Plus 250GB", "Composite Temperature"),
        ("Samsung SSD 970 EVO Plus 250GB", "Temperature #1"),
        ("Samsung SSD 970 EVO Plus 250GB", "Temperature #2"),
        ("CT500P310SSD8", "Composite Temperature"),
        ("CT500P310SSD8", "Temperature #1"),
        ("Samsung SSD 860 QVO 1TB ", "Temperature"),
    ]
    .iter()
    .map(|(h, s)| (h.to_string(), s.to_string()))
    .collect()
}

fn by_title<'a>(list: &'a [Component], title: &str) -> &'a Component {
    list.iter()
        .find(|c| c.title == title)
        .unwrap_or_else(|| panic!("no component {title}"))
}

fn names(c: &Component) -> Vec<&str> {
    let mut v: Vec<&str> = c.sensors.iter().map(|(_, s)| s.as_str()).collect();
    v.sort();
    v
}

#[test]
fn each_part_of_the_pc_is_its_own_component_in_a_familiar_order() {
    let list = components(&real_machine());
    let titles: Vec<&str> = list.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Processeur",
            "Carte graphique",
            "Carte mère",
            "Boîtier",
            "Disque 1",
            "Disque 2",
            "Disque 3"
        ]
    );
}

#[test]
fn the_processor_probe_of_the_motherboard_goes_with_the_processor() {
    let list = components(&real_machine());
    assert_eq!(
        names(by_title(&list, "Processeur")),
        ["CCD1 (Tdie)", "CPU", "Core (Tctl/Tdie)"]
    );
    assert!(!names(by_title(&list, "Carte mère")).contains(&"CPU"));
}

#[test]
fn the_case_has_its_own_component_apart_from_the_motherboard() {
    let list = components(&real_machine());
    assert_eq!(names(by_title(&list, "Boîtier")), ["System"]);
    assert_eq!(
        names(by_title(&list, "Carte mère")),
        ["Chipset", "VRM MOS", "VSoC MOS"]
    );
    assert_eq!(by_title(&list, "Boîtier").subtitle, None);
}

#[test]
fn the_disks_are_numbered_by_model_and_keep_their_own_sensors() {
    let list = components(&real_machine());
    let model = |t: &str| by_title(&list, t).subtitle.clone();
    assert_eq!(model("Disque 1").as_deref(), Some("CT500P310SSD8"));
    assert_eq!(
        model("Disque 2").as_deref(),
        Some("Samsung SSD 860 QVO 1TB")
    );
    assert_eq!(
        model("Disque 3").as_deref(),
        Some("Samsung SSD 970 EVO Plus 250GB")
    );
    assert_eq!(names(by_title(&list, "Disque 3")).len(), 3);
}

#[test]
fn the_model_shown_is_the_hardware_not_the_board_that_reports_a_probe() {
    let list = components(&real_machine());
    assert_eq!(
        by_title(&list, "Processeur").subtitle.as_deref(),
        Some("AMD Ryzen 7 5700X")
    );
}

#[test]
fn a_single_disk_or_card_is_not_numbered() {
    let one = vec![(
        "Samsung SSD 990".to_string(),
        "Composite Temperature".to_string(),
    )];
    assert_eq!(components(&one)[0].title, "Disque");
    let gpu = vec![("NVIDIA RTX 4070".to_string(), "GPU Core".to_string())];
    assert_eq!(components(&gpu)[0].title, "Carte graphique");
}

#[test]
fn each_component_is_stood_for_by_its_most_telling_sensor() {
    let list = components(&real_machine());
    let main = |t: &str| by_title(&list, t).main.1.clone();
    assert_eq!(main("Processeur"), "Core (Tctl/Tdie)");
    assert_eq!(main("Carte graphique"), "GPU Core");
    assert_eq!(main("Carte mère"), "Chipset");
    assert_eq!(main("Boîtier"), "System");
    assert_eq!(main("Disque 3"), "Composite Temperature");
    assert_eq!(main("Disque 2"), "Temperature");
}

#[test]
fn the_numbering_does_not_depend_on_the_order_the_sensors_arrive_in() {
    let mut reversed = real_machine();
    reversed.reverse();
    assert_eq!(components(&real_machine()), components(&reversed));
}

#[test]
fn duplicated_sensors_are_counted_once() {
    let mut twice = real_machine();
    twice.extend(real_machine());
    assert_eq!(components(&twice), components(&real_machine()));
}

#[test]
fn unknown_hardware_keeps_its_own_name() {
    let other = vec![("Pump XYZ".to_string(), "Water".to_string())];
    let list = components(&other);
    assert_eq!(list[0].title, "Pump XYZ");
    assert_eq!(list[0].kind, Kind::Other);
    assert_eq!(list[0].subtitle, None);
}

#[test]
fn hardware_is_recognised_by_its_sensors_then_by_its_name() {
    assert_eq!(hardware_kind("Whatever", &["GPU Core"]), Kind::Gpu);
    assert_eq!(
        hardware_kind("Intel Core i7", &["CPU Package", "Core #1"]),
        Kind::Cpu
    );
    assert_eq!(hardware_kind("ASUS ROG STRIX", &["CPU"]), Kind::Board);
    assert_eq!(hardware_kind("WD Blue SN570", &["Temperature"]), Kind::Disk);
    assert_eq!(
        hardware_kind("MSI MAG B660", &["Temperature #1"]),
        Kind::Board
    );
    // A graphics card made by a board maker is still a graphics card.
    assert_eq!(hardware_kind("Gigabyte RTX 4070", &["GPU Core"]), Kind::Gpu);
}

#[test]
fn sensor_names_are_readable_and_keep_the_technical_name() {
    assert_eq!(
        sensor_name("Core (Tctl/Tdie)", Kind::Cpu),
        (
            "Cœurs du processeur".to_string(),
            Some("Core (Tctl/Tdie)".to_string())
        )
    );
    assert_eq!(
        sensor_name("Temperature #2", Kind::Disk),
        (
            "Sonde interne n° 2".to_string(),
            Some("Temperature #2".to_string())
        )
    );
    assert_eq!(
        sensor_name("Mystery", Kind::Other),
        ("Mystery".to_string(), None)
    );
}

#[test]
fn names_with_non_ascii_text_do_not_break_the_matching() {
    let odd = vec![("Carte é".to_string(), "Température é".to_string())];
    assert_eq!(components(&odd).len(), 1);
}
