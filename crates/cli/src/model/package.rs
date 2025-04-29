use std::path::{Path, PathBuf};
use serde_json::Value;
use std::fs;
use std::process::Command;
use std::env;

use crate::{service::script::ScriptService, util::{linker::link, logger::log_verbose}};

#[derive(Debug, Default, Clone)]
pub struct Scripts {
    pub preinstall: Option<String>,
    pub install: Option<String>,
    pub postinstall: Option<String>,
    pub prepare: Option<String>,
    pub preprepare: Option<String>,
    pub postprepare: Option<String>,
    pub prepublish: Option<String>,
}

impl Scripts {
    pub fn has_any_script(&self) -> bool {
        self.preinstall.is_some() || self.install.is_some() || self.postinstall.is_some()
    }

    pub fn get_script(&self, script_type: &str) -> Option<&String> {
        match script_type {
            "preinstall" => self.preinstall.as_ref(),
            "install" => self.install.as_ref(),
            "postinstall" => self.postinstall.as_ref(),
            "prepare" => self.prepare.as_ref(),
            "preprepare" => self.preprepare.as_ref(),
            "postprepare" => self.postprepare.as_ref(),
            "prepublish" => self.prepublish.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub path: PathBuf,
    pub bin_files: Vec<(String, String)>, // (bin_name, relative_path)
    pub scripts: Scripts,
    #[allow(dead_code)]
    pub scope: Option<String>, // exp "@babel"
    pub fullname: String, // exp "@babel/parser"
    #[allow(dead_code)]
    pub name: String, // exp parser
    pub version: String,
}

impl PackageInfo {
    pub fn get_bin_dir(&self) -> Option<PathBuf> {
        match self
            .path
            .ancestors()
            .find(|p| p.ends_with("node_modules"))
            .map(|p| p.to_path_buf().join(".bin"))
        {
            Some(path) => Some(path),
            None => Some(PathBuf::from("node_modules/.bin")),
        }
    }

    #[allow(dead_code)]
    pub fn has_bin_files(&self) -> bool {
        !self.bin_files.is_empty()
    }

    #[allow(dead_code)]
    pub fn needs_processing(&self) -> bool {
        self.has_bin_files() || self.scripts.has_any_script()
    }

    pub fn has_script(&self) -> bool {
        self.scripts.has_any_script()
    }

    pub fn from_path(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        // Read package.json
        let package_json_path = path.join("package.json");
        let content = fs::read_to_string(&package_json_path)?;
        let data: Value = serde_json::from_str(&content)?;

        // Parse package name
        let name = data["name"].as_str()
            .ok_or_else(|| "Failed to get package name from package.json")?
            .to_string();

        // Parse version
        let version = data["version"].as_str()
            .ok_or_else(|| "Failed to get package version from package.json")?
            .to_string();

        // Parse scope
        let scope = if name.starts_with('@') {
            name.split('/').next().map(|s| s.to_string())
        } else {
            None
        };

        // Parse binary files
        let bin_files = if let Some(bin) = data.get("bin") {
            if bin.is_object() {
                bin.as_object()
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                            .collect()
                    })
                    .unwrap_or_default()
            } else if bin.is_string() {
                let bin_path = bin.as_str().unwrap_or_default().to_string();
                vec![(name.clone(), bin_path)]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Parse scripts
        let scripts = Scripts {
            preinstall: data["scripts"]["preinstall"].as_str().map(String::from),
            install: data["scripts"]["install"].as_str().map(String::from),
            postinstall: data["scripts"]["postinstall"].as_str().map(String::from),
            prepare: data["scripts"]["prepare"].as_str().map(String::from),
            preprepare: data["scripts"]["preprepare"].as_str().map(String::from),
            postprepare: data["scripts"]["postprepare"].as_str().map(String::from),
            prepublish: data["scripts"]["prepublish"].as_str().map(String::from),
        };

        Ok(PackageInfo {
            path: path.to_path_buf(),
            bin_files,
            scripts,
            name: name.clone(),
            fullname: name,
            version,
            scope,
        })
    }

    pub async fn link_to_global(&self, global_bin_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure bin directory exists
        tokio::fs::create_dir_all(&global_bin_dir).await?;

        // Link each binary file
        for (bin_name, relative_path) in &self.bin_files {
            let target_path = self.path.join(relative_path);
            let link_path = global_bin_dir.join(bin_name);

            log_verbose(&format!("Linking global binary: {} -> {}", bin_name, relative_path));

            // Ensure target file is executable
            ScriptService::ensure_executable(&target_path).await?;

            // Create symbolic link
            link(&target_path, &link_path)?;
        }

        // Update PATH environment variable for current process
        if let Ok(current_path) = env::var("PATH") {
            let global_bin_str = global_bin_dir.to_string_lossy().to_string();
            if !current_path.contains(&global_bin_str) {
                let new_path = format!("{}:{}", global_bin_str, current_path);
                env::set_var("PATH", new_path);
                log_verbose("Updated PATH environment variable");
            }
        }

        Ok(())
    }

}
