use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::fs;
use tokio;
use serde_json::{json, Value};
use crate::util::logger::{log_info, log_verbose};
use crate::util::cache::parse_pattern;
use crate::util::registry::resolve;
use crate::util::downloader::download;
use crate::util::cloner::clone;
use crate::cmd::install::install;
use crate::model::package::PackageInfo;

/// Get the utoo cache directory (~/.utoo/utx)
pub fn get_utoo_cache_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Unable to find home directory"))?;
    Ok(home_dir.join(".utoo").join("utx"))
}

/// Convert package name to safe directory name
/// Examples:
/// - "cowsay" -> "cowsay"
/// - "@modelcontextprotocol/create-server" -> "@modelcontextprotocol_create-server"
fn package_name_to_dir_name(package_name: &str) -> String {
    package_name.replace("/", "_")
}

/// Install a package to the utoo cache directory using utoo's own installation logic
/// This function is similar to prepare_global_package_json but installs to ~/.utoo/utx
pub async fn install_package_to_cache(package_name: &str) -> Result<PathBuf> {
    // Parse package name and version
    let (name, _version, version_spec) = parse_package_spec(package_name).await?;

    let cache_dir = get_utoo_cache_dir()?;

    // Create a unique directory for this package installation
    let package_cache_dir = cache_dir.join(format!("{}@{}", package_name_to_dir_name(&name), _version));

    // Check if already installed (package.json exists)
    let package_json_path = package_cache_dir.join("lib/node_modules/").join(&name);
    if package_json_path.exists() {
        log_verbose(&format!("Package {} already cached at {}", name, package_cache_dir.display()));
        return Ok(package_cache_dir);
    }

    log_info(&format!("Installing package {} to cache using utoo...", name));

    // Create the cache directory
    tokio::fs::create_dir_all(&package_cache_dir).await?;

    // Get package info from registry
    let resolved = resolve(&name, &version_spec).await?;

    // Get tarball URL from manifest
    let tarball_url = resolved.manifest["dist"]["tarball"]
        .as_str()
        .ok_or_else(|| anyhow!("Failed to get tarball URL from manifest"))?;

    // Download and extract package to a temporary location
    let cache_dir_global = crate::util::cache::get_cache_dir();
    let cache_path = cache_dir_global.join(format!("{}/{}", name, resolved.version));
    let cache_flag_path = cache_dir_global.join(format!("{}/{}/_resolved", name, resolved.version));

    // Download if not cached
    if !cache_flag_path.exists() {
        log_verbose(&format!(
            "Downloading {} to {}",
            tarball_url,
            cache_path.display()
        ));
        download(tarball_url, &cache_path)
            .await
            .map_err(|e| anyhow!("Failed to download package: {}", e))?;

        // If the package has install scripts, create a flag file
        if resolved.manifest.get("hasInstallScript") == Some(&json!(true)) {
            let has_install_script_flag_path = cache_path.join("_hasInstallScript");
            fs::write(has_install_script_flag_path, "")?;
        }
    }

    // Clone to package directory
    log_verbose(&format!(
        "Cloning {} to {}",
        cache_path.display(),
        package_cache_dir.display()
    ));
    clone(&cache_path, &package_cache_dir, true)
        .await
        .map_err(|e| anyhow!("Failed to clone package: {}", e))?;

    // Remove devDependencies, peerDependencies and optionalDependencies from package.json
    let mut package_json: Value = serde_json::from_reader(fs::File::open(&package_json_path)?)?;

    // Remove specified dependency fields
    if let Some(obj) = package_json.as_object_mut() {
        obj.remove("devDependencies");
        obj.remove("peerDependencies");
        obj.remove("optionalDependencies");
    }

    // Write back the modified package.json
    fs::write(
        &package_json_path,
        serde_json::to_string_pretty(&package_json)?,
    )?;

    // Install dependencies if any
    if package_json.get("dependencies").is_some() {
        log_verbose("Installing package dependencies using utoo...");
        install(true, &package_cache_dir).await?; // ignore_scripts = true for execute packages
    }

    // Create bin links
    log_verbose("Creating bin links...");
    create_bin_links(&package_cache_dir).await?;

    log_info(&format!("Package {} installed successfully using utoo", name));
    Ok(package_cache_dir)
}

/// Create bin links in node_modules/.bin directory
async fn create_bin_links(package_cache_dir: &PathBuf) -> Result<()> {
    // Create PackageInfo from the installed package
    let package_info = PackageInfo::from_path(package_cache_dir)?;

    // If the package has bin files, create the node_modules/.bin directory and links
    if !package_info.bin_files.is_empty() {
        let bin_dir = package_cache_dir.join("bin");
        tokio::fs::create_dir_all(&bin_dir).await?;

        for (bin_name, relative_path) in &package_info.bin_files {
            let target_path = package_cache_dir.join(relative_path);
            let link_path = bin_dir.join(bin_name);

            log_verbose(&format!(
                "Linking binary: {} -> {}",
                bin_name, relative_path
            ));

            // Ensure target file is executable
            crate::service::script::ScriptService::ensure_executable(&target_path).await?;

            // Create symbolic link
            crate::util::linker::link(&target_path, &link_path)?;
        }

        log_verbose(&format!("Created {} bin links", package_info.bin_files.len()));
    }

    Ok(())
}

/// Parse package spec, similar to the one in helper/lock.rs
async fn parse_package_spec(spec: &str) -> Result<(String, String, String)> {
    let (name, version_spec) = parse_pattern(spec);
    let resolved = resolve(&name, &version_spec).await?;
    Ok((name, resolved.version, version_spec))
}
