use glob::glob;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::helper::lock::{extract_package_name, Package};
use crate::helper::workspace;
use crate::helper::{is_cpu_compatible, is_os_compatible};
use crate::util::cloner::clone;
use crate::util::downloader::download;
use crate::util::linker::link;
use crate::util::logger::{log_progress, log_verbose, PROGRESS_BAR};

use super::binary::update_package_binary;

async fn clean_deps(
    groups: &HashMap<usize, Vec<(String, Package)>>,
    cwd: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut valid_packages = std::collections::HashSet::new();
    for (_, packages) in groups {
        for (path, _) in packages {
            valid_packages.insert(path.clone());
        }
    }

    log_verbose(&format!("Valid packages: {:?}", valid_packages));

    let mut node_modules_dirs = vec![cwd.join("node_modules")];

    let workspaces = workspace::find_workspaces(&cwd.to_path_buf()).await?;
    for (_, path, _) in workspaces {
        let workspace_node_modules = path.join("node_modules");
        if workspace_node_modules.exists() {
            node_modules_dirs.push(workspace_node_modules);
        }
    }

    // cleanup unused packages in all workspace_members
    for node_modules in node_modules_dirs {
        let pattern = node_modules
            .join("**/package.json")
            .to_string_lossy()
            .to_string();
        for entry in glob(&pattern)? {
            if let Ok(path) = entry {
                let pkg_dir = path.parent().unwrap();
                let relative_path = pkg_dir.strip_prefix(&node_modules)?;
                let pkg_name = relative_path.to_string_lossy().to_string();

                // ignore package.json in dist directory
                // exp: node_modules/react/dist/package.json
                let parts: Vec<&str> = pkg_name.split('/').collect();
                if parts.len() > 2 || (parts.len() == 2 && !parts[0].starts_with('@')) {
                    continue;
                }

                let node_modules_prefix = node_modules
                    .strip_prefix(cwd)?
                    .to_string_lossy()
                    .to_string();
                let full_pkg_name = format!("{}/{}", node_modules_prefix, pkg_name);

                if !valid_packages.contains(&full_pkg_name) {
                    log_verbose(&format!("Cleaning unused package: {}", full_pkg_name));
                    if let Err(e) = tokio::fs::remove_dir_all(pkg_dir).await {
                        log_verbose(&format!("Failed to remove {}: {}", full_pkg_name, e));
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn install_packages(
    groups: &HashMap<usize, Vec<(std::string::String, Package)>>,
    cache_dir: &Path,
    cwd: &Path,
    semaphore: Arc<Semaphore>,
) -> Result<(), Box<dyn std::error::Error>> {
    // clean unused deps
    clean_deps(groups, cwd).await?;

    let mut depths: Vec<_> = groups.keys().cloned().collect();
    depths.sort_unstable();

    for depth in depths.iter() {
        if let Some(packages) = groups.get(depth) {
            let mut tasks: Vec<
                tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
            > = Vec::new();
            for (path, package) in packages.iter() {
                let path = path.clone();
                let package = package.clone();
                if let Some(resolved) = package.resolved {
                    if package.link.is_some() {
                        let link_name = extract_package_name(&path);
                        if link_name.is_empty() {
                            PROGRESS_BAR.inc(1);
                            log_verbose(&format!(
                                "Link skipped due to empty package name: {}",
                                path
                            ));
                            log_progress(&format!("empty package name link skipped"));
                            continue;
                        }
                        log_verbose(&format!("Attempting to link from {} to {}", resolved, path));
                        if let Err(e) = link(Path::new(&resolved), Path::new(&path)) {
                            log_verbose(&format!(
                                "Link failed: source={}, target={}, error={}",
                                resolved, path, e
                            ));
                            return Err(format!("Link failed: {}", e).into());
                        }
                        PROGRESS_BAR.inc(1);
                        log_progress(&format!("resolved link skipped",));
                        continue;
                    }

                    // skip when cpu or os is not compatible
                    if package.cpu.is_some() {
                        if !is_cpu_compatible(&package.cpu.unwrap()) {
                            PROGRESS_BAR.inc(1);
                            log_verbose(&format!("cpu skipped: {}", &path));
                            log_progress(&format!("uncompatibel cpu skipped",));
                            continue;
                        }
                    }

                    if package.os.is_some() {
                        if !is_os_compatible(&package.os.unwrap()) {
                            PROGRESS_BAR.inc(1);
                            log_verbose(&format!("os skipped: {}", &path));
                            log_progress(&format!("uncompatibel os skipped",));
                            continue;
                        }
                    }

                    let name = extract_package_name(&path);
                    let version = package.version.as_ref().unwrap();
                    let cache_path = cache_dir.join(format!("{}/{}", name, version));
                    let cache_flag_path = cache_dir.join(format!("{}/{}/_resolved", name, version));
                    let cwd_clone = cwd.to_path_buf();
                    let should_resolve = !cache_flag_path.exists();
                    let semaphore = Arc::clone(&semaphore);

                    let task = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.unwrap();
                        if should_resolve {
                            log_progress(&format!("Downloading {} to {}", path, name));
                            log_verbose(&format!("Downloading {} to {}", path, name));
                            match download(&resolved, &cache_path).await {
                                Ok(_) => {
                                    log_progress(&format!("{} downloaded", name));
                                    log_verbose(&format!("{} downloaded", name));
                                }
                                Err(e) => {
                                    log_verbose(&format!(
                                        "Download failed: source={}, target={}, error={}",
                                        resolved,
                                        cache_path.display(),
                                        e
                                    ));
                                    return Err(Box::new(std::io::Error::new(
                                        std::io::ErrorKind::Other,
                                        format!("{} download failed: {}", name, e),
                                    ))
                                        as Box<dyn std::error::Error + Send + Sync>);
                                }
                            }
                        }

                        log_verbose(&format!("{} clone", name));
                        match clone(&cache_path, &cwd_clone.join(&path), true).await {
                            Ok(_) => {
                                log_verbose(&format!("{} resolved", name));
                                PROGRESS_BAR.inc(1);
                                log_progress(&format!("{} resolved", name));
                                update_package_binary(&cwd_clone.join(&path), &name).await?;
                                Ok(())
                            }
                            Err(e) => Err(format!(
                                "Copy failed {} to {}: {}",
                                cache_path.display(),
                                cwd_clone.join(&path).display(),
                                e
                            )
                            .into()),
                        }
                    });
                    tasks.push(task);
                } else {
                    PROGRESS_BAR.inc(1);
                    log_progress(&format!("{} no resolved info skipped", path));
                }
            }

            for task in tasks {
                match task.await {
                    Ok(Ok(())) => continue,
                    Ok(Err(e)) => {
                        log_verbose(&format!("Task execution error: {}", e));
                        return Err(format!("Error during installation: {}", e).into());
                    }
                    Err(e) => {
                        log_verbose(&format!("Task join error: {}", e));
                        return Err(format!("Task execution failed: {}", e).into());
                    }
                }
            }
        }
    }

    Ok(())
}
