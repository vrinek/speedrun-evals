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

/// Explains why `model_bytes` of model will not fit in the GPUs' free memory,
/// or `None` if it fits (or there is no GPU and the run is CPU-only anyway).
/// Without room the runtime quietly keeps part of the model on the CPU, and the
/// result would be a slow number filed under a fast GPU. Loading it could also
/// push out whatever else is using that memory.
pub fn gpu_shortfall(devices: &str, model_bytes: u64) -> Option<String> {
    let gpus: Vec<_> = device_lines(devices).collect();
    if gpus.is_empty() {
        return None;
    }
    let free: f64 = gpus.iter().map(|(_, _, free)| free).sum();
    // ponytail: weights plus a fifth, plus 1 GiB for context and scratch buffers.
    // A rule of thumb for the fixed 8192-token context; measure if it misjudges.
    let needed = (model_bytes as f64 / (1 << 20) as f64 * 1.2 + 1024.0).round();
    (free < needed).then(|| {
        format!(
            "the model needs about {needed} MiB of GPU memory but only {free} MiB free was found. \
             Another program is probably using the GPU; close it and try again."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUSY: &str = "Available devices:\n  \
        Vulkan0: AMD Radeon 8060S Graphics (RADV STRIX_HALO) (96512 MiB, 9692 MiB free)\n";

    #[test]
    fn a_model_that_fits_in_free_gpu_memory_is_fine() {
        // 2.5 GB model beside a large resident one: 10% free is still plenty.
        assert_eq!(gpu_shortfall(BUSY, 2_497_280_256), None);
    }

    #[test]
    fn a_model_larger_than_free_gpu_memory_is_refused() {
        // 8 GiB of weights needs about 10.6 GiB with working memory.
        let why = gpu_shortfall(BUSY, 8 << 30).unwrap();
        assert!(why.contains("9692 MiB free") && why.contains("10854 MiB"), "{why}");
    }

    #[test]
    fn free_memory_adds_up_across_gpus_and_cpu_only_runs_are_not_checked() {
        let two = "  Vulkan0: A (8192 MiB, 6000 MiB free)\n  Vulkan1: B (8192 MiB, 6000 MiB free)\n";
        assert_eq!(gpu_shortfall(two, 8 << 30), None);
        assert_eq!(gpu_shortfall("Available devices:\n  (none)\n", 80 << 30), None);
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
