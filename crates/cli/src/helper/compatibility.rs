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
                    if os_str.starts_with('!') {
                        normalize_os(&os_str[1..]) != current_os
                    } else {
                        normalize_os(os_str) == current_os
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
            if cpu_name.starts_with('!') {
                !is_arch_match(&cpu_name[1..])
            } else {
                is_arch_match(&cpu_name)
            }
        }
        Value::Array(cpu_list) => cpu_list.iter().any(|cpu| {
            if let Some(cpu_str) = cpu.as_str() {
                let cpu_str = cpu_str.to_lowercase();
                if cpu_str.starts_with('!') {
                    !is_arch_match(&cpu_str[1..])
                } else {
                    is_arch_match(&cpu_str)
                }
            } else {
                false
            }
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
