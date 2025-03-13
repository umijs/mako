use crate::helper::compatibility::{is_cpu_compatible, is_os_compatible};
use crate::helper::env::get_node_abi;
use crate::helper::package::parse_package_name;
use crate::model::package::{PackageInfo, Scripts};
use crate::util::cache::get_cache_dir;
use crate::util::cloner::clone;
use crate::util::logger::{
    finish_progress_bar, log_info, log_progress, log_verbose, log_warning, start_progress_bar,
    PROGRESS_BAR,
};
use std::path::{Path, PathBuf};

use std::collections::HashMap;

use futures::future::join_all;
use serde_json::Value;
use std::fs;
use tokio::task;

use super::script::ScriptService;

pub struct PackageService;

impl PackageService {
    pub async fn process_project_hooks() -> Result<(), String> {
        let content = fs::read_to_string("package.json")
            .map_err(|e| format!("Failed to read package.json: {}", e))?;

        let data: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse package.json: {}", e))?;

        let binding = serde_json::Map::new();
        let scripts = data
            .get("scripts")
            .and_then(|s| s.as_object())
            .unwrap_or(&binding);

        let hooks = [
            "preinstall",
            "install",
            "postinstall",
            "prepublish",
            "preprepare",
            "prepare",
            "postprepare",
        ];

        let (scope, name, fullname) = parse_package_name(&format!(
            "node_modules/{}",
            data.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        ));

        let package_info = PackageInfo {
            path: PathBuf::from("."),
            bin_files: Vec::new(),
            scripts: Scripts {
                preinstall: scripts
                    .get("preinstall")
                    .and_then(|s| s.as_str())
                    .map(String::from),
                install: scripts
                    .get("install")
                    .and_then(|s| s.as_str())
                    .map(String::from),
                postinstall: scripts
                    .get("postinstall")
                    .and_then(|s| s.as_str())
                    .map(String::from),
                prepare: scripts
                    .get("prepare")
                    .and_then(|s| s.as_str())
                    .map(String::from),
                preprepare: scripts
                    .get("preprepare")
                    .and_then(|s| s.as_str())
                    .map(String::from),
                postprepare: scripts
                    .get("postprepare")
                    .and_then(|s| s.as_str())
                    .map(String::from),
                prepublish: scripts
                    .get("prepublish")
                    .and_then(|s| s.as_str())
                    .map(String::from),
            },
            name,
            fullname,
            version: data
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            scope,
        };

        for hook in hooks {
            if let Some(_) = scripts.get(hook).and_then(|s| s.as_str()) {
                log_info(&format!("Executing project hook: {}", hook));
                ScriptService::execute_script(&package_info, hook, true).await?;
            }
        }

        Ok(())
    }
    pub fn collect_packages() -> Result<Vec<PackageInfo>, String> {
        log_verbose("Collecting packages...");
        let lock_file = fs::read_to_string("package-lock.json")
            .map_err(|e| format!("Failed to load package-lock.json: {}", e))?;

        let lock_data: Value = serde_json::from_str(&lock_file)
            .map_err(|e| format!("Failed to parse package-lock.json: {}", e))?;

        let mut packages = Vec::new();
        if let Some(deps) = lock_data.get("packages").and_then(|v| v.as_object()) {
            for (path, info) in deps {
                if path == "" {
                    continue;
                }
                if let Some(package) = Self::process_package_info(path, info)? {
                    packages.push(package);
                }
            }
        }
        Ok(packages)
    }

    pub fn create_execution_queues(
        packages: Vec<PackageInfo>,
    ) -> Result<Vec<Vec<PackageInfo>>, String> {
        log_verbose("Prepareing execute queues...");
        let mut queues = vec![Vec::new(); 5];

        // create queues, and we will check if there is a cache first
        // if there is a cache, we will not execute the scripts related tasks
        for package in packages {
            let has_cached = Self::has_cached(&package);
            if has_cached {
                log_verbose(&format!(
                    "Package {} is cached, skipping execution",
                    package.fullname
                ));
                queues[0].push(package.clone());
            }
            if package.scripts.preinstall.is_some() && !has_cached {
                log_verbose(&format!(
                    "Adding {} to preinstall queue",
                    package.path.display()
                ));
                queues[1].push(package.clone());
            }
            if !package.bin_files.is_empty() {
                log_verbose(&format!(
                    "Adding {} to bin linking queue",
                    package.path.display()
                ));
                queues[2].push(package.clone());
            }
            if package.scripts.install.is_some() && !has_cached {
                log_verbose(&format!(
                    "Adding {} to install queue",
                    package.path.display()
                ));
                queues[3].push(package.clone());
            }
            if package.scripts.postinstall.is_some() && !has_cached {
                log_verbose(&format!(
                    "Adding {} to postinstall queue",
                    package.path.display()
                ));
                queues[4].push(package.clone());
            }
        }

        log_verbose(&format!(
            "Queue creation completed, {} tasks pending",
            queues.iter().map(|q| q.len()).sum::<usize>()
        ));

        Ok(queues)
    }

    pub fn process_package_info(path: &str, info: &Value) -> Result<Option<PackageInfo>, String> {
        let info = match info.as_object() {
            Some(obj) => obj,
            None => return Ok(None),
        };

        // check if there is an install script or bin files
        let has_install_script = info
            .get("hasInstallScript")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let has_bin = info.get("bin").is_some();

        if !has_install_script && !has_bin {
            return Ok(None);
        }

        // check if the package is compatible in current os & cpu
        if let Some(os_constraint) = info.get("os") {
            if !is_os_compatible(os_constraint) {
                log_verbose(&format!(
                    "Package {} is not compatible with current OS, skipped",
                    path
                ));
                return Ok(None);
            }
        }

        if let Some(cpu_constraint) = info.get("cpu") {
            if !is_cpu_compatible(cpu_constraint) {
                log_verbose(&format!(
                    "Package {} is not compatible with current CPU architecture, skipped",
                    path
                ));
                return Ok(None);
            }
        }

        let (scope, name, fullname) = parse_package_name(path);
        let bin_files = Self::parse_bin_files(info.get("bin"), &name);
        let package_path = if path.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(path)
        };

        let scripts = Self::read_package_scripts(&package_path)?;

        Ok(Some(PackageInfo {
            path: package_path,
            bin_files,
            scripts,
            scope,
            fullname,
            name,
            version: info
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        }))
    }

    fn parse_bin_files(bin: Option<&Value>, package_name: &str) -> Vec<(String, String)> {
        match bin {
            Some(bin) => {
                if bin.is_object() {
                    bin.as_object()
                        .map(|obj| {
                            obj.iter()
                                .map(|(k, v)| {
                                    (k.clone(), v.as_str().unwrap_or_default().to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                } else if bin.is_string() {
                    let bin_path = bin.as_str().unwrap_or_default().to_string();
                    vec![(package_name.to_string(), bin_path)]
                } else {
                    Vec::new()
                }
            }
            None => Vec::new(),
        }
    }

    fn get_package_cache_dir(package: &PackageInfo) -> PathBuf {
        let cache_dir = get_cache_dir();
        let node_abi = get_node_abi(&package.path).unwrap_or_else(|_| "unknown".to_string());

        PathBuf::from(cache_dir)
            .join(&package.fullname)
            .join(&package.version)
            .join(".utoo_builded")
            .join(&node_abi)
    }

    fn has_cached(package: &PackageInfo) -> bool {
        if !package.has_script() {
            return false;
        }
        let target_dir = Self::get_package_cache_dir(package);
        target_dir.exists()
    }

    async fn store_build_result(package: &PackageInfo) {
        if Self::has_cached(package) {
            return;
        }
        let target_dir = Self::get_package_cache_dir(package);

        match clone(&package.path, &target_dir, false).await {
            Ok(_) => log_info(&format!(
                "Cached build result for package {}/{}, will be reused later",
                package.fullname, package.version
            )),
            Err(e) => log_warning(&format!(
                "Failed to cache package {}: {}",
                package.fullname, e
            )),
        }
    }

    async fn restore_build_result(package: &PackageInfo) -> Result<(), String> {
        if !package.has_script() {
            return Ok(());
        }
        let target_dir = Self::get_package_cache_dir(package);
        match clone(&target_dir, &package.path, false).await {
            Ok(_) => Ok(()),
            Err(e) => {
                log_warning(&format!(
                    "Failed to restore package {} from cache: {}",
                    package.fullname, e
                ));
                Err(format!("Failed to restore cache: {}", e))
            }
        }
    }

    fn read_package_scripts(package_path: &Path) -> Result<Scripts, String> {
        let package_json_path = package_path.join("package.json");
        let content = fs::read_to_string(&package_json_path)
            .map_err(|e| format!("Failed to read {}: {}", package_json_path.display(), e))?;

        let data: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", package_json_path.display(), e))?;

        let binding = serde_json::Map::new();
        let scripts = data
            .get("scripts")
            .and_then(|s| s.as_object())
            .unwrap_or(&binding);

        Ok(Scripts {
            preinstall: scripts
                .get("preinstall")
                .and_then(|s| s.as_str())
                .map(String::from),
            install: scripts
                .get("install")
                .and_then(|s| s.as_str())
                .map(String::from),
            postinstall: scripts
                .get("postinstall")
                .and_then(|s| s.as_str())
                .map(String::from),
            prepare: None,
            preprepare: None,
            postprepare: None,
            prepublish: None,
        })
    }

    pub async fn execute_queues(queues: Vec<Vec<PackageInfo>>) -> Result<(), String> {
        let mut all_packages = Vec::new(); // collect all packages in queue

        // collect all tasks in queue
        let total_tasks: usize = queues.iter().map(|q| q.len()).sum();
        PROGRESS_BAR.set_length(total_tasks as u64);
        start_progress_bar();

        for (i, queue) in queues.into_iter().enumerate() {
            let phase_name = match i {
                0 => "restore cache",
                1 => "preinstall",
                2 => "bin files",
                3 => "install",
                4 => "postinstall",
                _ => unreachable!(),
            };

            if i != 0 && i != 2 {
                // collect scripts related tasks
                all_packages.extend(queue.clone());
            }

            let tasks: Vec<_> = queue
                .into_iter()
                .map(|package| {
                    task::spawn(async move {
                        let result = match i {
                            0 => Self::restore_build_result(&package).await,
                            1 => ScriptService::execute_script(&package, "preinstall", false).await,
                            2 => ScriptService::link_bin_files(&package).await,
                            3 => ScriptService::execute_script(&package, "install", false).await,
                            4 => {
                                ScriptService::execute_script(&package, "postinstall", false).await
                            }
                            _ => unreachable!(),
                        };

                        PROGRESS_BAR.inc(1);
                        log_progress(&format!(
                            "{} / {}@{} / {:?}",
                            phase_name, &package.fullname, &package.version, &package.path
                        ));

                        result
                    })
                })
                .collect();

            let results = join_all(tasks).await;
            for result in results {
                match result {
                    Ok(task_result) => {
                        if let Err(e) = task_result {
                            return Err(format!("Task execution failed: {}", e));
                        }
                    }
                    Err(e) => return Err(format!("Task execution failed: {}", e)),
                }
            }
        }

        // store build result for unique packages after all queues are executed
        let unique_packages = all_packages
            .into_iter()
            .map(|p| ((p.fullname.clone(), p.version.clone()), p))
            .collect::<HashMap<(String, String), PackageInfo>>()
            .into_values();

        for package in unique_packages {
            Self::store_build_result(&package).await;
        }
        finish_progress_bar("hook scripts completed");
        Ok(())
    }
}
