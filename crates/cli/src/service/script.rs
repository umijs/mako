use crate::model::package::PackageInfo;
use crate::util::logger::log_verbose;
use std::env;
use std::os::unix::fs::{symlink as unix_symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;

pub struct ScriptService;

impl ScriptService {
    pub async fn execute_script(
        package: &PackageInfo,
        script_type: &str,
        show_output: bool,
    ) -> Result<(), String> {
        let script = package.scripts.get_script(script_type);

        if let Some(script) = script {
            log_verbose(&format!(
                "Executing {} script for {}: {}",
                script_type,
                package.path.display(),
                script
            ));

            let bin_paths = Self::collect_bin_paths(package);
            let env_path = Self::build_path_env(&bin_paths);

            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg(script)
                .current_dir(&package.path)
                .env("PATH", env_path)
                .env("npm_lifecycle_event", script_type)
                .env(
                    "INIT_CWD",
                    env::current_dir()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                )
                .env(
                    "npm_package_json",
                    package.path.join("package.json").display().to_string(),
                )
                .env("npm_config_prefix", "")
                .env("npm_config_global", "false");

            log_verbose(&format!("Executing command: {:?}", cmd));

            let output = tokio::process::Command::from(cmd)
                .output()
                .await
                .map_err(|e| format!("Failed to execute script: {}", e))?;

            if show_output && !output.stdout.is_empty() {
                println!("{}", String::from_utf8_lossy(&output.stdout));
            }

            if !output.status.success() {
                return Err(format!(
                    "Script execution failed: {}\n{}",
                    String::from_utf8_lossy(&output.stderr),
                    String::from_utf8_lossy(&output.stdout)
                ));
            }
        }

        Ok(())
    }

    pub async fn link_bin_files(package: &PackageInfo) -> Result<(), String> {
        let bin_dir = package.get_bin_dir().ok_or_else(|| {
            format!(
                "Cannot find node_modules directory: {}",
                package.path.display()
            )
        })?;

        fs::create_dir_all(&bin_dir)
            .await
            .map_err(|e| format!("Failed to create .bin directory: {}", e))?;

        for (bin_name, relative_path) in &package.bin_files {
            Self::process_bin_file(package, bin_dir.as_path(), bin_name, &relative_path).await?;
        }

        Ok(())
    }

    async fn process_bin_file(
        package: &PackageInfo,
        bin_dir: &Path,
        bin_name: &str,
        relative_path: &str,
    ) -> Result<(), String> {
        let target_path = package.path.join(relative_path);
        let bin_path = bin_dir.join(bin_name);

        log_verbose(&format!(
            "Processing binary file: {} -> {}",
            bin_name, relative_path
        ));

        Self::ensure_executable(&target_path).await?;
        Self::create_symlink(package, &bin_path, relative_path)?;

        Ok(())
    }

    async fn ensure_executable(target_path: &Path) -> Result<(), String> {
        let permissions = tokio::fs::metadata(&target_path)
            .await
            .map_err(|e| {
                format!(
                    "Failed to get file permissions {}: {}",
                    target_path.display(),
                    e
                )
            })?
            .permissions();

        let is_executable = permissions.mode() & 0o111 != 0;

        if !is_executable {
            let mut content = fs::read_to_string(&target_path)
                .await
                .map_err(|e| format!("Failed to read file {}: {}", target_path.display(), e))?;

            if !content.starts_with("#!") {
                content = format!("#!/usr/bin/env node\n{}", content);
                fs::write(&target_path, content).await.map_err(|e| {
                    format!("Failed to write shebang {}: {}", target_path.display(), e)
                })?;
            }
        }

        let mut perms = permissions;
        perms.set_mode(0o755);
        fs::set_permissions(&target_path, perms)
            .await
            .map_err(|e| format!("Failed to set executable permissions: {}", e))?;

        Ok(())
    }

    fn create_symlink(
        package: &PackageInfo,
        bin_path: &Path,
        relative_path: &str,
    ) -> Result<(), String> {
        let node_modules_count = bin_path
            .components()
            .filter(|c| c.as_os_str() == "node_modules")
            .count();

        let prefix = "../".repeat(node_modules_count);
        let relative_target = format!("../{}{}/{}", prefix, package.path.display(), relative_path);

        if let Err(e) = unix_symlink(&relative_target, bin_path) {
            if e.raw_os_error() != Some(17) {
                // EEXIST = 17
                return Err(format!(
                    "Failed to create symlink {} -> {:?}: {}",
                    bin_path.display(),
                    relative_target,
                    e
                ));
            }
            log_verbose(&format!(
                "Link already exists, skipping: {} -> {:?}",
                bin_path.display(),
                relative_target
            ));
        } else {
            log_verbose(&format!(
                "Successfully created link: {} -> {:?}",
                bin_path.display(),
                relative_target
            ));
        }

        Ok(())
    }

    fn collect_bin_paths(package: &PackageInfo) -> Vec<PathBuf> {
        let mut bin_paths = Vec::new();
        let mut current_path = package.path.clone();

        while let Some(parent) = current_path.parent() {
            let bin_path = parent.join("node_modules/.bin");
            if bin_path.exists() {
                if let Ok(absolute_path) = std::fs::canonicalize(&bin_path) {
                    bin_paths.push(absolute_path);
                }
            }
            current_path = parent.to_path_buf();
        }

        bin_paths
    }

    fn build_path_env(bin_paths: &[PathBuf]) -> String {
        let path_separator = ":";
        let original_path = env::var("PATH").unwrap_or_default();
        let additional_paths = bin_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(path_separator);

        format!(
            "{}{}{}",
            additional_paths,
            if additional_paths.is_empty() {
                ""
            } else {
                path_separator
            },
            original_path
        )
    }
}
