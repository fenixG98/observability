use observability::collect::temps::*;

fn raw(v: &[(&str, Option<f32>)]) -> Vec<(String, Option<f32>)> {
    v.iter().map(|(l, t)| (l.to_string(), *t)).collect()
}

fn pairs(s: &[observability::model::TempSensor]) -> Vec<(String, f32)> {
    s.iter().map(|t| (t.label.clone(), t.celsius)).collect()
}

#[test]
fn groups_real_linux_labels() {
    let input = raw(&[
        ("coretemp Package id 0", Some(76.0)),
        ("coretemp Core 0", Some(72.0)),
        ("coretemp Core 8", Some(76.0)),
        ("nvme Composite KBG6AZNV1T02 LA KIOXIA", Some(43.85)),
        ("nvme Sensor 1 KBG6AZNV1T02 LA KIOXIA", Some(43.85)),
        ("acpitz temp1", Some(74.0)),
        ("iwlwifi_1 temp1", Some(62.0)),
        ("thinkpad temp4", Some(0.0)),
        ("thinkpad GPU", None),
        ("thinkpad CPU", Some(74.0)),
    ]);
    assert_eq!(
        pairs(&group_sensors(&input)),
        vec![
            ("CPU".into(), 76.0),
            ("Nuclis".into(), 76.0),
            ("Disc".into(), 43.85),
            ("ACPI".into(), 74.0),
            ("WiFi".into(), 62.0),
        ]
    );
}

#[test]
fn groups_apple_silicon_and_gpu_labels() {
    let input = raw(&[
        ("PMU tdie1", Some(45.0)),
        ("PMU tdie2", Some(48.0)),
        ("PMU tdev1", Some(38.0)),
        ("NAND CH0 temp", Some(35.0)),
        ("gas gauge battery", Some(30.0)),
        ("GPU NVIDIA GeForce RTX 3060", Some(41.0)),
    ]);
    assert_eq!(
        pairs(&group_sensors(&input)),
        vec![
            ("SoC".into(), 48.0),
            ("GPU".into(), 41.0),
            ("Disc".into(), 35.0),
            ("Dispositius".into(), 38.0),
            ("Bateria".into(), 30.0),
        ]
    );
}

#[test]
fn drops_impossible_values_and_keeps_unknown_labels() {
    let input = raw(&[
        ("pch_cannonlake temp1", Some(51.0)),
        ("weird", Some(400.0)),
        ("nan", Some(f32::NAN)),
    ]);
    assert_eq!(
        pairs(&group_sensors(&input)),
        vec![("pch_cannonlake temp1".into(), 51.0)]
    );
}
