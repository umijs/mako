use serde_json::Value;
use std::env::consts::{ARCH, OS};

fn normalize_os(os: &str) -> String {
    match os.to_lowercase().as_str() {
        "darwin" | "macos" => "macos".to_string(),
        other => other.to_string(),
    }
}

pub fn is_os_compatible(constraint: &Value) -> bool {
    match constraint {
        Value::String(os_name) => normalize_os(os_name) == normalize_os(OS),
        Value::Array(os_list) => {
            let current_os = normalize_os(OS);
            os_list.iter().any(|os| {
                if let Some(os_str) = os.as_str() {
                    let matches =
                        normalize_os(os_str.strip_prefix('!').unwrap_or(os_str)) == current_os;
                    if os_str.starts_with('!') {
                        !matches
                    } else {
                        matches
                    }
                } else {
                    false
                }
            })
        }
        _ => true, // ignore when format is incorrect
    }
}

pub fn is_cpu_compatible(constraint: &Value) -> bool {
    match constraint {
        Value::String(cpu_name) => {
            let cpu_name = cpu_name.to_lowercase();
            let arch = cpu_name.strip_prefix('!').unwrap_or(&cpu_name);
            let matches = is_arch_match(arch);
            if cpu_name.starts_with('!') {
                !matches
            } else {
                matches
            }
        }
        Value::Array(cpu_list) => cpu_list.iter().any(|cpu| {
            cpu.as_str().map_or(false, |cpu_str| {
                let cpu_str = cpu_str.to_lowercase();
                let arch = cpu_str.strip_prefix('!').unwrap_or(&cpu_str);
                let matches = is_arch_match(arch);
                if cpu_str.starts_with('!') {
                    !matches
                } else {
                    matches
                }
            })
        }),
        _ => true, // ignore when format is incorrect
    }
}

fn is_arch_match(arch: &str) -> bool {
    let current_arch = ARCH.to_lowercase();
    match arch {
        "arm64" => current_arch == "aarch64",
        "aarch64" => current_arch == "aarch64",
        _ => arch == current_arch,
    }
}
