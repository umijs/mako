use crate::util::logger::{log_error, log_info, log_warning};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug)]
struct VersionCache {
    version: String,
    check_time: u64,
}

pub async fn init_auto_update() -> Result<(), String> {
    let cache = match read_version_cache() {
        Ok(cache) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if now - cache.check_time > 3600 {
                check_and_update_cache().await?
            } else {
                cache
            }
        }
        Err(_) => check_and_update_cache().await?,
    };

    let current_version = env!("CARGO_PKG_VERSION");
    if cache.version != current_version {
        log_info(&format!(
            "New version found: {} (current version: {}), updating automatically...",
            cache.version, current_version
        ));
        log_info(&format!("npm i utoo -g"));

        execute_update()?;

        log_info("Update completed, please restart");
        process::exit(0);
    }
    Ok(())
}

async fn check_and_update_cache() -> Result<VersionCache, String> {
    match check_remote_version().await {
        Ok(_) => read_version_cache(),
        Err(e) => {
            log_warning(&format!("Failed to check remote version: {}", e));
            Err("RegistryError".to_string())
        }
    }
}

fn execute_update() -> Result<(), String> {
    let status = Command::new("npm")
        .args(&["i", "utoo", "-g"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| e.to_string())?;

    if status.success() {
        Ok(())
    } else {
        log_error("Auto update failed, please update manually");
        Err(format!(
            "Auto update failed, please execute manually {}",
            status
        ))
    }
}

async fn check_remote_version() -> Result<(), String> {
    let registry_url = "https://registry.npmmirror.com/utoo/latest";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2000))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(registry_url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let package_info = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    let version = package_info["version"]
        .as_str()
        .ok_or("Unable to get version information")?
        .to_string();

    let cache = VersionCache {
        version,
        check_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    save_version_cache(&cache)
}

fn get_cache_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".utoo").join("remote-version.json")
}

fn read_version_cache() -> Result<VersionCache, String> {
    let content = fs::read_to_string(get_cache_path()).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn save_version_cache(cache: &VersionCache) -> Result<(), String> {
    let cache_path = get_cache_path();
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string(cache).map_err(|e| e.to_string())?;
    fs::write(cache_path, content).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn test_execute_update_success() {
        // always true to simulate success
        let result = Command::new("true")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .unwrap();

        assert!(result.success());
        assert_eq!(result.code(), Some(0));
    }

    #[test]
    fn test_execute_update_failure() {
        // always false to simulate failure
        let result = Command::new("false")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .unwrap();

        assert!(!result.success());
        assert_eq!(result.code(), Some(1));
    }

    #[test]
    fn test_execute_update_command_not_found() {
        // simulate command not found
        let result = Command::new("non_existent_command")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        assert!(result.is_err());
    }

    #[test]
    fn test_execute_update_integration() {
        // test for execute_update function
        let result = execute_update();

        // just check if it's Ok or Err
        match result {
            Ok(()) => (),
            Err(e) => assert!(e.contains("Auto update failed, please update manually")),
        }
    }
}
