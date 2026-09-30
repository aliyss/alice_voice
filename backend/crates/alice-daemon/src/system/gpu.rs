//! Graphics devices of the machine.
//!
//! A graphics device is what makes a built in model fast, so the settings
//! page shows which devices the daemon can reach and how much memory each
//! one holds. The probe reads the sources the machine already carries and
//! installs nothing: `nvidia-smi` answers for a card of NVIDIA with its
//! own memory, the kernel reports a card of AMD under `/sys/class/drm` with
//! its own memory, and `lspci` names a card that the kernel leaves unnamed.
//!
//! A device that reports no memory of its own is an integrated device: it
//! draws on the memory of the system and has no memory of its own, so the
//! probe reports it as shared rather than as a device without a number.
//! That is what every card of Intel reports, and what a machine without
//! `nvidia-smi` reports for a card of NVIDIA.
//!
//! A machine without a graphics device reports no device at all. What the
//! daemon can run on is a question of its own, and the device probe of the
//! runtime answers it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The largest number of devices the probe reports.
const MAX_DEVICES: usize = 8;

/// The program that reports the cards of NVIDIA.
const NVIDIA_SMI: &str = "nvidia-smi";

/// The fields one NVIDIA card reports: name, memory, and driver.
const NVIDIA_QUERY: &str = "name,memory.total,memory.used,memory.free,driver_version";

/// The program that names a device of the bus.
const LSPCI: &str = "lspci";

/// The directory that lists the graphics devices of the kernel.
const DRM_DIR: &str = "/sys/class/drm";

/// The directory the kernel keeps the loaded modules under.
const MODULE_DIR: &str = "/sys/module";

/// The identifier of a memory value the machine does not report.
const NOT_AVAILABLE: &str = "[N/A]";

/// The part of a PCI address the tools leave out, the domain.
const PCI_DOMAIN: &str = "0000:";

/// One graphics device of the machine.
#[derive(Clone, Debug, PartialEq)]
pub struct Gpu {
    /// The name of the device, as the machine reports it.
    pub name: String,
    /// The maker of the device: `nvidia`, `amd`, or `intel`.
    pub vendor: String,
    /// True when the device has no memory of its own and draws on the
    /// memory of the system.
    pub shared_memory: bool,
    /// The memory of the device in bytes, or null when the device has none
    /// of its own or the machine does not report it.
    pub memory_total_bytes: Option<u64>,
    /// The memory in use in bytes, or null.
    pub memory_used_bytes: Option<u64>,
    /// The memory free in bytes, or null.
    pub memory_free_bytes: Option<u64>,
    /// The version of the driver, or null when the machine does not name
    /// one.
    pub driver_version: Option<String>,
}

/// Read the graphics devices of the machine.
///
/// The tool of NVIDIA answers with the memory of every card, so it is
/// asked first. A machine without it falls back to the kernel, which names
/// the card and reports the memory of a card of AMD.
pub fn gpus() -> Vec<Gpu> {
    let mut devices = nvidia_gpus();
    if devices.is_empty() {
        devices = kernel_gpus();
    }
    devices.truncate(MAX_DEVICES);
    devices
}

/// Read the cards of NVIDIA with the tool of the driver.
fn nvidia_gpus() -> Vec<Gpu> {
    let output = match Command::new(NVIDIA_SMI)
        .arg(format!("--query-gpu={NVIDIA_QUERY}"))
        .arg("--format=csv,noheader,nounits")
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    parse_nvidia(&String::from_utf8_lossy(&output.stdout))
}

/// Read one card out of every line of the report of `nvidia-smi`.
///
/// A line carries five comma separated fields in the order of the query.
/// A line with fewer fields, and a line without a name, is not a card.
fn parse_nvidia(output: &str) -> Vec<Gpu> {
    output.lines().filter_map(parse_nvidia_line).collect()
}

/// Read one card out of one line of the report of `nvidia-smi`.
fn parse_nvidia_line(line: &str) -> Option<Gpu> {
    let fields: Vec<&str> = line.split(',').map(str::trim).collect();
    if fields.len() < 5 || fields[0].is_empty() {
        return None;
    }
    let total = mib(fields[1]);
    Some(Gpu {
        name: fields[0].to_string(),
        vendor: "nvidia".to_string(),
        // A card of NVIDIA holds its own memory. A driver that reports no
        // memory leaves the card without a number rather than shared.
        shared_memory: false,
        memory_total_bytes: total,
        memory_used_bytes: mib(fields[2]),
        memory_free_bytes: mib(fields[3]),
        driver_version: named(fields[4]),
    })
}

/// Read a memory value the tool reported in mebibytes as bytes.
///
/// A card the driver does not report the memory of answers `[N/A]`, which
/// is no number rather than a number of zero.
fn mib(value: &str) -> Option<u64> {
    value.parse::<u64>().ok().map(|value| value * 1024 * 1024)
}

/// Read the cards of the kernel out of the device directories.
///
/// The kernel lists one directory per card and one per render node, and
/// both point at the same device, so the devices are read by the path of
/// the device itself and each card is read once.
fn kernel_gpus() -> Vec<Gpu> {
    let Ok(entries) = std::fs::read_dir(DRM_DIR) else {
        return Vec::new();
    };
    let names = lspci_names();

    let mut devices: BTreeMap<PathBuf, Gpu> = BTreeMap::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        // A connector of a card is named after the card with a dash, so
        // the two are told apart by the dash.
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let Ok(device) = std::fs::canonicalize(entry.path().join("device")) else {
            continue;
        };
        let Some(gpu) = device_of(&device, &names) else {
            continue;
        };
        devices.entry(device).or_insert(gpu);
    }
    devices.into_values().collect()
}

/// Read one card out of the directory of its device.
///
/// A card reports no memory of its own when it is an integrated card, and
/// the flag says so rather than leaving the reader with a number that is
/// missing for a reason the page cannot explain.
fn device_of(device: &Path, names: &BTreeMap<String, String>) -> Option<Gpu> {
    let id = read_trim(&device.join("vendor"))?;
    let vendor = vendor_name(&id);
    let driver = driver_of(device);
    let total = read_number(&device.join("mem_info_vram_total"));
    let used = read_number(&device.join("mem_info_vram_used"));
    Some(Gpu {
        name: device_name(device, &id, &vendor, driver.as_deref(), names),
        vendor,
        shared_memory: total.is_none(),
        memory_total_bytes: total,
        memory_used_bytes: used,
        // The kernel reports the total and the use of a card of AMD, so
        // the free memory is the difference between the two.
        memory_free_bytes: total
            .zip(used)
            .map(|(total, used)| total.saturating_sub(used)),
        driver_version: driver.as_deref().and_then(driver_version),
    })
}

/// The name of one device, as the machine reports it.
///
/// The kernel names a card of AMD itself, `lspci` names the rest, and a
/// machine without either falls back to the driver the kernel loaded for
/// the device, which is a name the user can still look up.
fn device_name(
    device: &Path,
    id: &str,
    vendor: &str,
    driver: Option<&str>,
    names: &BTreeMap<String, String>,
) -> String {
    if let Some(product) = read_trim(&device.join("product_name")) {
        return product;
    }
    if let Some(slot) = pci_slot(device) {
        if let Some(name) = names.get(&slot) {
            return name.clone();
        }
    }
    match driver {
        Some(driver) => format!("{vendor} {driver}"),
        None => format!("{vendor} {id}"),
    }
}

/// Read the name of every device of the bus with `lspci`.
///
/// One call names every card the machine holds, so the probe asks once
/// rather than once per device. A machine without the tool, and a tool that
/// does not answer, names nothing and the caller falls back.
fn lspci_names() -> BTreeMap<String, String> {
    let output = match Command::new(LSPCI).output() {
        Ok(output) if output.status.success() => output,
        _ => return BTreeMap::new(),
    };
    parse_lspci(&String::from_utf8_lossy(&output.stdout))
}

/// Read one name out of every line of the report of `lspci`.
///
/// A line reads as `00:02.0 VGA compatible controller: Intel Corporation
/// Meteor Lake-P [Intel Arc Graphics] (rev 08)`, so the slot is the first
/// word and the name follows the first colon. The revision and the class
/// after the name are dropped, because the page names the device and not
/// the revision of its silicon.
fn parse_lspci(output: &str) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    for line in output.lines() {
        let Some((head, rest)) = line.split_once(": ") else {
            continue;
        };
        let Some(slot) = head.split_whitespace().next() else {
            continue;
        };
        let name = rest.split(" (rev").next().unwrap_or(rest).trim();
        if !name.is_empty() {
            names.insert(slot.to_string(), name.to_string());
        }
    }
    names
}

/// The slot of one device on the bus, as `lspci` writes it.
fn pci_slot(device: &Path) -> Option<String> {
    let text = std::fs::read_to_string(device.join("uevent")).ok()?;
    let slot = text
        .lines()
        .find_map(|line| line.strip_prefix("PCI_SLOT_NAME="))?
        .trim();
    // `lspci` leaves the domain out of a common address, and the kernel
    // writes it, so the two are read alike.
    Some(slot.strip_prefix(PCI_DOMAIN).unwrap_or(slot).to_string())
}

/// The maker of a device, by the identifier the kernel reports.
fn vendor_name(id: &str) -> String {
    match id.trim().to_lowercase().as_str() {
        "0x10de" => "nvidia".to_string(),
        "0x1002" | "0x1022" => "amd".to_string(),
        "0x8086" => "intel".to_string(),
        other => other.to_string(),
    }
}

/// The name of the driver the kernel loaded for one device.
fn driver_of(device: &Path) -> Option<String> {
    let text = std::fs::read_to_string(device.join("uevent")).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("DRIVER="))
        .map(|driver| driver.trim().to_string())
        .filter(|driver| !driver.is_empty())
}

/// The version of one loaded module, when the module reports one.
///
/// A driver that is part of the kernel carries no version file, so a
/// version of null means the machine does not name one.
fn driver_version(driver: &str) -> Option<String> {
    read_trim(&Path::new(MODULE_DIR).join(driver).join("version"))
}

/// Read one file as trimmed text. A missing file reads as null.
fn read_trim(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Read one file as a number. A missing file reads as null.
fn read_number(path: &Path) -> Option<u64> {
    read_trim(path)?.parse().ok()
}

/// Read one field of the report of the tool, where `[N/A]` is no value.
fn named(value: &str) -> Option<String> {
    if value.is_empty() || value == NOT_AVAILABLE {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_of_nvidia_reads_with_its_memory() {
        let report = "NVIDIA GeForce RTX 4090, 24564, 1024, 23540, 550.54.14\n";
        let devices = parse_nvidia(report);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "NVIDIA GeForce RTX 4090");
        assert_eq!(devices[0].vendor, "nvidia");
        assert!(!devices[0].shared_memory);
        assert_eq!(devices[0].memory_total_bytes, Some(24_564 * 1024 * 1024));
        assert_eq!(devices[0].memory_used_bytes, Some(1024 * 1024 * 1024));
        assert_eq!(devices[0].memory_free_bytes, Some(23_540 * 1024 * 1024));
        assert_eq!(devices[0].driver_version.as_deref(), Some("550.54.14"));
    }

    #[test]
    fn two_cards_read_as_two_devices() {
        let report =
            "NVIDIA A100, 40960, 0, 40960, 550.54.14\nNVIDIA T4, 15360, 0, 15360, 550.54.14\n";
        assert_eq!(parse_nvidia(report).len(), 2);
    }

    #[test]
    fn a_memory_the_driver_does_not_report_reads_as_nothing() {
        let report = "NVIDIA GeForce GT 1030, [N/A], [N/A], [N/A], 550.54.14\n";
        let devices = parse_nvidia(report);

        assert_eq!(devices[0].memory_total_bytes, None);
        assert_eq!(devices[0].memory_free_bytes, None);
    }

    #[test]
    fn a_line_without_a_name_reads_as_no_card() {
        assert!(parse_nvidia(", 24564, 0, 24564, 550.54.14\n").is_empty());
        assert!(parse_nvidia("NVIDIA T4, 15360\n").is_empty());
        assert!(parse_nvidia("").is_empty());
    }

    #[test]
    fn the_report_of_the_bus_names_a_card_by_its_slot() {
        let report = "00:02.0 VGA compatible controller: Intel Corporation Meteor Lake-P [Intel Arc Graphics] (rev 08)\n00:1f.3 Audio device: Intel Corporation Device 7e28\n";
        let names = parse_lspci(report);

        assert_eq!(
            names.get("00:02.0").map(String::as_str),
            Some("Intel Corporation Meteor Lake-P [Intel Arc Graphics]")
        );
        assert_eq!(
            names.get("00:1f.3").map(String::as_str),
            Some("Intel Corporation Device 7e28")
        );
    }

    #[test]
    fn a_line_of_the_bus_without_a_name_reads_as_nothing() {
        assert!(parse_lspci("no colon here\n").is_empty());
        assert!(parse_lspci("").is_empty());
    }

    #[test]
    fn the_maker_reads_out_of_the_identifier_of_the_kernel() {
        assert_eq!(vendor_name("0x10DE"), "nvidia");
        assert_eq!(vendor_name(" 0x1002 "), "amd");
        assert_eq!(vendor_name("0x8086"), "intel");
        assert_eq!(vendor_name("0x1234"), "0x1234");
    }

    #[test]
    fn a_card_without_a_name_falls_back_to_its_driver() {
        let device = Path::new("/nonexistent-device");
        let name = device_name(device, "0x8086", "intel", Some("i915"), &BTreeMap::new());
        assert_eq!(name, "intel i915");

        let name = device_name(device, "0x8086", "intel", None, &BTreeMap::new());
        assert_eq!(name, "intel 0x8086");
    }
}
