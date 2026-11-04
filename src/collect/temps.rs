//! Grouping and normalization of temperature sensors (Linux hwmon, macOS SMC/IOHID).

use crate::model::TempSensor;

/// Display order of the known groups; the rest come after, by temperature.
const ORDER: [&str; 5] = ["CPU", "Nuclis", "SoC", "GPU", "Disc"];

/// Visible group for a raw sensor label.
pub fn group_label(label: &str) -> String {
    let l = label.to_lowercase();
    let group = if l.contains("package id")
        || l.starts_with("k10temp tctl")
        || l.starts_with("k10temp tdie")
        || l.starts_with("zenpower")
        || l == "thinkpad cpu"
        || l == "cpu"
        || l.starts_with("cpu ")
    {
        "CPU"
    } else if l.starts_with("coretemp core") {
        "Nuclis"
    } else if l.starts_with("amdgpu")
        || l.starts_with("nouveau")
        || l.starts_with("radeon")
        || l.contains("gpu")
    {
        "GPU"
    } else if l.starts_with("nvme") || l.starts_with("nand") || l.contains("ssd") {
        "Disc"
    } else if l.starts_with("iwlwifi")
        || l.starts_with("ath")
        || l.starts_with("mt7")
        || l.contains("wifi")
    {
        "WiFi"
    } else if l.starts_with("acpitz") {
        "ACPI"
    } else if l.contains("battery") || l.starts_with("bat") {
        "Bateria"
    } else if l.contains("tdie") {
        "SoC"
    } else if l.contains("tdev") {
        "Dispositius"
    } else {
        return label.to_string();
    };
    group.to_string()
}

/// Groups (maximum per group), discards impossible values and sorts.
pub fn group_sensors(raw: &[(String, Option<f32>)]) -> Vec<TempSensor> {
    let mut groups: Vec<TempSensor> = Vec::new();
    for (label, t) in raw {
        let Some(t) = *t else { continue };
        if !t.is_finite() || t <= 0.0 || t > 150.0 {
            continue;
        }
        let g = group_label(label);
        match groups.iter_mut().find(|s| s.label == g) {
            Some(s) => s.celsius = s.celsius.max(t),
            None => groups.push(TempSensor {
                label: g,
                celsius: t,
            }),
        }
    }
    let rank = |l: &str| ORDER.iter().position(|o| *o == l).unwrap_or(ORDER.len());
    groups.sort_by(|a, b| {
        rank(&a.label)
            .cmp(&rank(&b.label))
            .then(b.celsius.total_cmp(&a.celsius))
    });
    groups
}
