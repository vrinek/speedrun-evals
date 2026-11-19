//! Detects the machine's hardware so nobody has to type it in.
//! The parsers are pure; `detect` does the reading.

use speedrun_core::{Gpu, Hardware};
use std::{fs, process::Command};

pub fn detect() -> Result<Hardware, String> {
    let read = |path: &str| fs::read_to_string(path).map_err(|e| format!("{path}: {e}"));
    let lspci = Command::new("lspci")
        .arg("-Dmm")
        .output()
        .map_err(|e| format!("could not run lspci: {e}"))?;
    let gpus = parse_gpus(&String::from_utf8_lossy(&lspci.stdout))
        .into_iter()
        .map(|(slot, name)| {
            // ponytail: amdgpu and friends expose this; the proprietary NVIDIA driver
            // does not, so those cards report no VRAM until we parse nvidia-smi.
            let vram = fs::read_to_string(format!("/sys/bus/pci/devices/{slot}/mem_info_vram_total"));
            Gpu { name, vram_gb: vram.ok().and_then(|bytes| vram_gb(&bytes)) }
        })
        .collect();
    Ok(Hardware {
        cpu: parse_cpu(&read("/proc/cpuinfo")?),
        ram_gb: parse_ram_gb(&read("/proc/meminfo")?)?,
        gpus,
        os: format!("Linux {}", read("/proc/sys/kernel/osrelease")?.trim()),
    })
}

/// The first `model name` in /proc/cpuinfo.
pub fn parse_cpu(cpuinfo: &str) -> String {
    cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name")?.split_once(':'))
        .map_or("Unknown CPU".to_string(), |(_, name)| name.trim().to_string())
}

/// Installed RAM in GB. The kernel reports a little less than what is installed,
/// so round up to the next multiple of 4 to make identical machines agree.
pub fn parse_ram_gb(meminfo: &str) -> Result<u32, String> {
    let kb: u64 = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:")?.trim().strip_suffix("kB"))
        .and_then(|kb| kb.trim().parse().ok())
        .ok_or("no MemTotal in /proc/meminfo")?;
    Ok((kb.div_ceil(4 * 1024 * 1024) * 4) as u32)
}

/// `(pci slot, display name)` for every graphics device in `lspci -Dmm` output.
pub fn parse_gpus(lspci: &str) -> Vec<(String, String)> {
    lspci
        .lines()
        .filter_map(|line| {
            let (slot, rest) = line.split_once(' ')?;
            // Fields are quoted: class, vendor, device, ...
            let fields: Vec<&str> = rest.split('"').skip(1).step_by(2).collect();
            let [class, vendor, device, ..] = fields[..] else { return None };
            if !["VGA", "3D", "Display"].iter().any(|kind| class.contains(kind)) {
                return None;
            }
            let vendor = ["NVIDIA", "AMD", "Intel"]
                .into_iter()
                .find(|short| vendor.contains(short))
                .unwrap_or(vendor);
            // pci.ids puts the marketing name in brackets: `GA102 [GeForce RTX 3090]`.
            // ponytail: one PCI id can cover several products (see the Strix Halo
            // test case); telling them apart needs a per-vendor lookup.
            let device = device
                .split_once('[')
                .and_then(|(_, name)| name.strip_suffix(']'))
                .unwrap_or(device);
            Some((slot.to_string(), format!("{vendor} {device}")))
        })
        .collect()
}

/// Dedicated VRAM in GB from a sysfs byte count. Integrated GPUs report a token
/// amount (they use system RAM), which is reported as no dedicated VRAM.
pub fn vram_gb(bytes: &str) -> Option<u32> {
    let gb = (bytes.trim().parse::<u64>().ok()? as f64 / (1u64 << 30) as f64).round() as u32;
    (gb >= 2).then_some(gb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_cpu_model_name() {
        let cpuinfo = "processor\t: 0\nmodel name\t: AMD RYZEN AI MAX+ 395 w/ Radeon 8060S\nprocessor\t: 1\nmodel name\t: other\n";
        assert_eq!(parse_cpu(cpuinfo), "AMD RYZEN AI MAX+ 395 w/ Radeon 8060S");
        assert_eq!(parse_cpu("processor\t: 0\n"), "Unknown CPU");
    }

    #[test]
    fn rounds_ram_up_to_the_installed_size() {
        assert_eq!(parse_ram_gb("MemTotal:       131156428 kB\nMemFree: 1 kB\n"), Ok(128));
        assert_eq!(parse_ram_gb("MemTotal:       16004128 kB\n"), Ok(16));
        assert_eq!(parse_ram_gb("MemTotal:       65700000 kB\n"), Ok(64));
        assert!(parse_ram_gb("MemFree: 1 kB\n").is_err());
    }

    #[test]
    fn finds_graphics_devices_and_names_them() {
        let lspci = r#"0000:00:00.0 "Host bridge" "Advanced Micro Devices, Inc. [AMD]" "Device 1507" -p00 "" ""
0000:01:00.0 "VGA compatible controller" "NVIDIA Corporation" "GA102 [GeForce RTX 3090]" -ra1 -p00 "ASUSTeK Computer Inc." "Device 87b3"
0000:c3:00.0 "Display controller" "Advanced Micro Devices, Inc. [AMD/ATI]" "Strix Halo [Radeon Graphics / Radeon 8050S Graphics / Radeon 8060S Graphics]" -rc1 -p00 "Framework Computer Inc." "Device 000a"
0000:05:00.0 "3D controller" "Intel Corporation" "Device 56a0" -r08 "" ""
"#;
        assert_eq!(
            parse_gpus(lspci),
            vec![
                ("0000:01:00.0".to_string(), "NVIDIA GeForce RTX 3090".to_string()),
                (
                    "0000:c3:00.0".to_string(),
                    "AMD Radeon Graphics / Radeon 8050S Graphics / Radeon 8060S Graphics".to_string()
                ),
                ("0000:05:00.0".to_string(), "Intel Device 56a0".to_string()),
            ]
        );
    }

    #[test]
    fn reports_dedicated_vram_only() {
        assert_eq!(vram_gb("25757220864\n"), Some(24));
        assert_eq!(vram_gb("536870912\n"), None);
        assert_eq!(vram_gb("garbage"), None);
    }
}
