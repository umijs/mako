use std::path::Path;
use serde_json::{Value, Map};
use tokio::fs;
use regex::Regex;
use semver::Version;
use tokio::sync::OnceCell;
use crate::util::config::get_registry;
use crate::util::logger::log_info;

#[derive(Debug)]
pub enum BinaryError {
    InvalidConfig(String),
    FileOperation(String),
    NetworkError(String),
    ParseError(String),
}

impl std::fmt::Display for BinaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryError::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
            BinaryError::FileOperation(msg) => write!(f, "File operation failed: {}", msg),
            BinaryError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            BinaryError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for BinaryError {}

static CONFIG: OnceCell<Value> = OnceCell::const_new();

async fn load_config() -> Result<&'static Value, BinaryError> {
    CONFIG.get_or_try_init(|| async {
        let registry = get_registry();
        let url = format!("{}/binary-mirror-config/latest", registry);
        let response = reqwest::get(&url)
            .await
            .map_err(|e| BinaryError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(BinaryError::NetworkError(format!("HTTP status: {}", response.status())));
        }

        response.json()
            .await
            .map_err(|e| BinaryError::ParseError(e.to_string()))
    }).await
}

fn update_binary_config(pkg: &mut Value, binary_mirror: &Map<String, Value>) {
    // Get existing binary configuration
    let mut new_binary = if let Some(binary) = pkg.get("binary") {
        if let Some(obj) = binary.as_object() {
            obj.clone()
        } else {
            Map::new()
        }
    } else {
        Map::new()
    };

    // Merge new configuration
    for (key, value) in binary_mirror {
        if key != "replaceHostFiles" {
            new_binary.insert(key.clone(), value.clone());
        }
    }

    // Update binary configuration
    pkg["binary"] = Value::Object(new_binary.clone());

    // Safely get package name and version
    let name = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
    let version = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("unknown");

    log_info(&format!("{}@{} download from binary mirror: {:?}",
        name,
        version,
        new_binary
    ));
}

async fn handle_node_pre_gyp_versioning(dir: &Path) -> Result<(), BinaryError> {
    let versioning_file = dir.join("node_modules/node-pre-gyp/lib/util/versioning.js");
    if versioning_file.exists() {
        let content = fs::read_to_string(&versioning_file)
            .await
            .map_err(|e| BinaryError::FileOperation(e.to_string()))?;

        let new_content = content.replace(
            "if (protocol === 'http:') {",
            "if (false && protocol === 'http:') { // hack by npminstall"
        );

        fs::write(&versioning_file, new_content)
            .await
            .map_err(|e| BinaryError::FileOperation(e.to_string()))?;
    }
    Ok(())
}

fn should_handle_replace_host(binary_mirror: &Map<String, Value>) -> bool {
    (binary_mirror.contains_key("replaceHost") && binary_mirror.contains_key("host")) ||
    binary_mirror.contains_key("replaceHostMap") ||
    binary_mirror.contains_key("replaceHostRegExpMap")
}

fn get_replace_host_files(binary_mirror: &Map<String, Value>) -> Vec<&str> {
    binary_mirror.get("replaceHostFiles")
        .and_then(|f| f.as_array())
        .map(|f| f.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_else(|| vec!["lib/index.js", "lib/install.js"])
}

fn replace_with_regex(content: &str, replace_map: &Value) -> Result<String, BinaryError> {
    let mut result = content.to_string();
    for (pattern, replacement) in replace_map.as_object().unwrap() {
        let re = Regex::new(pattern)
            .map_err(|e| BinaryError::ParseError(format!("Invalid regex pattern {}: {}", pattern, e)))?;
        result = re.replace_all(&result, replacement.as_str().unwrap()).to_string();
    }
    Ok(result)
}

fn replace_with_map(content: &str, binary_mirror: &Map<String, Value>) -> Result<String, BinaryError> {
    let replace_map = if let Some(map) = binary_mirror.get("replaceHostMap") {
        map.as_object().unwrap().clone()
    } else {
        let mut map = Map::new();
        let hosts = binary_mirror.get("replaceHost")
            .and_then(|h| h.as_array())
            .map(|h| h.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![binary_mirror["host"].as_str().unwrap()]);

        for host in hosts {
            map.insert(host.to_string(), Value::String(binary_mirror["host"].as_str().unwrap().to_string()));
        }
        map
    };

    let mut result = content.to_string();
    for (from, to) in replace_map {
        result = result.replace(&from, to.as_str().unwrap());
    }
    Ok(result)
}

async fn handle_replace_host(dir: &Path, binary_mirror: &Map<String, Value>) -> Result<(), BinaryError> {
    if !should_handle_replace_host(binary_mirror) {
        return Ok(());
    }

    let replace_host_files = get_replace_host_files(binary_mirror);
    for file in replace_host_files {
        let file_path = dir.join(file);
        if file_path.exists() {
            let content = fs::read_to_string(&file_path)
                .await
                .map_err(|e| BinaryError::FileOperation(e.to_string()))?;

            let new_content = if let Some(replace_map) = binary_mirror.get("replaceHostRegExpMap") {
                replace_with_regex(&content, replace_map)?
            } else {
                replace_with_map(&content, binary_mirror)?
            };

            fs::write(&file_path, new_content)
                .await
                .map_err(|e| BinaryError::FileOperation(e.to_string()))?;
        }
    }
    Ok(())
}

async fn handle_cypress(dir: &Path, pkg: &Value, binary_mirror: &Map<String, Value>) -> Result<(), BinaryError> {
    if pkg["name"].as_str().unwrap() != "cypress" {
        return Ok(());
    }

    let default_platforms = serde_json::json!({
        "darwin": "osx64",
        "linux": "linux64",
        "win32": "win64"
    });

    let platforms = if let Some(new_platforms) = binary_mirror.get("newPlatforms") {
        if Version::parse(pkg["version"].as_str().unwrap())
            .map(|v| v >= Version::parse("3.3.0").unwrap())
            .unwrap_or(false)
        {
            new_platforms
        } else {
            &default_platforms
        }
    } else {
        &default_platforms
    };

    if let Some(target_platform) = platforms[std::env::consts::OS].as_str() {
        let download_file = dir.join("lib/tasks/download.js");
        if download_file.exists() {
            let content = fs::read_to_string(&download_file)
                .await
                .map_err(|e| BinaryError::FileOperation(e.to_string()))?;

            let new_content = content
                .replace(
                    "return version ? prepend(`desktop/${version}`) : prepend('desktop')",
                    &format!("return \"{}\" + version + \"/{}/cypress.zip\"; // hack by npminstall",
                        binary_mirror["host"].as_str().unwrap(), target_platform)
                )
                .replace(
                    "return version ? prepend('desktop/' + version) : prepend('desktop');",
                    &format!("return \"{}\" + version + \"/{}/cypress.zip\"; // hack by npminstall",
                        binary_mirror["host"].as_str().unwrap(), target_platform)
                );

            fs::write(&download_file, new_content)
                .await
                .map_err(|e| BinaryError::FileOperation(e.to_string()))?;
        }
    }

    Ok(())
}

pub async fn update_package_binary(dir: &Path, name: &str) -> Result<(), BinaryError> {
    let config = load_config().await?;

    let mirrors = config["mirrors"]["china"].as_object()
        .ok_or_else(|| BinaryError::InvalidConfig("Invalid binary mirror config format".to_string()))?;

    if let Some(binary_mirror) = mirrors.get(name) {
        let binary_mirror = binary_mirror.as_object()
            .ok_or_else(|| BinaryError::InvalidConfig("Invalid binary mirror format".to_string()))?;

        // Read package.json
        let pkg_path = dir.join("package.json");
        let content = fs::read_to_string(&pkg_path)
            .await
            .map_err(|e| BinaryError::FileOperation(e.to_string()))?;

        let mut pkg: Value = serde_json::from_str(&content)
            .map_err(|e| BinaryError::ParseError(e.to_string()))?;

        // has install script and not replaceHostFiles
        let should_update_binary = if let Some(scripts) = pkg["scripts"].as_object() {
            scripts.contains_key("install") && !binary_mirror.contains_key("replaceHostFiles")
        } else {
            false
        };

        // detect node-pre-gyp
        let should_handle_node_pre_gyp = if let Some(scripts) = pkg["scripts"].as_object() {
            scripts.get("install")
                .and_then(|s| s.as_str())
                .map(|s| s.contains("node-pre-gyp install"))
                .unwrap_or(false)
        } else {
            false
        };

        // update binary config
        if should_update_binary {
            update_binary_config(&mut pkg, binary_mirror);
        }

        // process node-pre-gyp
        if should_handle_node_pre_gyp {
            handle_node_pre_gyp_versioning(dir).await?;
        }

        handle_replace_host(dir, binary_mirror).await?;
        handle_cypress(dir, &pkg, binary_mirror).await?;

        // Write updated package.json
        fs::write(pkg_path, serde_json::to_string_pretty(&pkg).unwrap())
            .await
            .map_err(|e| BinaryError::FileOperation(e.to_string()))?;
    }

    Ok(())
}
pub async fn get_envs() -> Option<&'static Map<String, Value>> {
    match load_config().await {
        Ok(_) => CONFIG.get()
            .and_then(|config| config["mirrors"]["china"]["ENVS"].as_object()),
        Err(_) => None,
    }
}
