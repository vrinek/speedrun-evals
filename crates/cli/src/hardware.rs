//! Detects the machine's hardware so nobody has to type it in.
//! The parsers are pure; `detect` does the reading.

use speedrun_core::{Gpu, Hardware};
use std::fs;

/// `devices` is the output of `llama-server --list-devices`: the GPUs recorded
/// are the ones the runtime can actually use, not merely the ones installed.
pub fn detect(devices: &str) -> Result<Hardware, String> {
    let read = |path: &str| fs::read_to_string(path).map_err(|e| format!("{path}: {e}"));
    Ok(Hardware {
        cpu: parse_cpu(&read("/proc/cpuinfo")?),
        ram_gb: parse_ram_gb(&read("/proc/meminfo")?)?,
        gpus: parse_gpus(devices),
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

/// GPUs from `llama-server --list-devices`, whose device lines look like
/// `  Vulkan0: AMD Radeon 8060S Graphics (RADV STRIX_HALO) (96512 MiB, 14011 MiB free)`.
/// For integrated GPUs the memory is the share of system RAM the driver may use.
pub fn parse_gpus(devices: &str) -> Vec<Gpu> {
    device_lines(devices)
        .map(|(name, total_mib, _)| Gpu {
            name: name.to_string(),
            vram_gb: Some((total_mib / 1024.0).round() as u32),
        })
        .collect()
}

/// `(name, total MiB, free MiB)` for each device line.
fn device_lines(devices: &str) -> impl Iterator<Item = (&str, f64, f64)> {
    devices.lines().filter_map(|line| {
        let (_, device) = line.trim().split_once(": ")?;
        let (name, memory) = device.rsplit_once(" (")?;
        let (total, free) = memory.split_once(" MiB, ")?;
        // Drop the driver note, e.g. ` (RADV STRIX_HALO)`.
        let name = name.split_once(" (").map_or(name, |(name, _)| name);
        Some((name, total.parse().ok()?, free.split_once(" MiB")?.0.parse().ok()?))
    })
}

/// Describes every GPU that another program is already using heavily: less
/// than three quarters of its memory free. A benchmark beside such a program
/// gives numbers nobody can compare, and may starve that program.
pub fn busy_gpus(devices: &str) -> Vec<String> {
    device_lines(devices)
        .filter(|(_, total, free)| *free < total * 0.75)
        .map(|(name, total, free)| format!("{name}: {free} of {total} MiB free"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gpu_with_most_memory_in_use_is_busy() {
        let devices = "Available devices:\n  \
            Vulkan0: AMD Radeon 8060S Graphics (RADV STRIX_HALO) (96512 MiB, 14011 MiB free)\n  \
            Vulkan1: NVIDIA GeForce RTX 3090 (24576 MiB, 23000 MiB free)\n";
        assert_eq!(
            busy_gpus(devices),
            ["AMD Radeon 8060S Graphics: 14011 of 96512 MiB free"]
        );
    }

    #[test]
    fn idle_or_absent_gpus_are_not_busy() {
        // A desktop session holding a quarter of the memory is still fine.
        assert!(busy_gpus("  Vulkan0: Some GPU (8192 MiB, 6144 MiB free)\n").is_empty());
        assert!(busy_gpus("Available devices:\n  (none)\n").is_empty());
    }

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
    fn reads_gpus_the_runtime_can_use() {
        let devices = "ggml_vulkan: Found 2 Vulkan devices\nAvailable devices:\n  \
            Vulkan0: AMD Radeon 8060S Graphics (RADV STRIX_HALO) (96512 MiB, 14011 MiB free)\n  \
            Vulkan1: NVIDIA GeForce RTX 3090 (24576 MiB, 24000 MiB free)\n";
        assert_eq!(
            parse_gpus(devices),
            vec![
                Gpu {
                    name: "AMD Radeon 8060S Graphics".into(),
                    vram_gb: Some(94)
                },
                Gpu {
                    name: "NVIDIA GeForce RTX 3090".into(),
                    vram_gb: Some(24)
                },
            ]
        );
    }

    #[test]
    fn no_devices_means_a_cpu_only_run() {
        assert_eq!(parse_gpus("Available devices:\n  (none)\n"), vec![]);
        assert_eq!(parse_gpus(""), vec![]);
    }
}
