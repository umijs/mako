use crate::helper::{package::serialize_tree_to_packages, ruborist::Ruborist};
use serde_json::json;
use semver::{Version, VersionReq};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

pub async fn build_deps() -> std::io::Result<()> {
    let path = PathBuf::from(".");
    let mut ruborist = Ruborist::new(path.clone());
    ruborist.build_ideal_tree().await?;

    // let _ = ruborist.print_tree();

    if let Some(ideal_tree) = &ruborist.ideal_tree {
        // Add reference
        // Create package-lock.json structure
        let lock_file = json!({
            "name": ideal_tree.name,  // Direct field access
            "version": ideal_tree.version,  // Direct field access
            "lockfileVersion": 3,
            "requires": true,
            "packages": serialize_tree_to_packages(ideal_tree),
        });

        // Write to temporary file first, then atomically move to target location
        let temp_path = path.join("package-lock.json.tmp");
        let target_path = path.join("package-lock.json");

        fs::write(&temp_path, serde_json::to_string_pretty(&lock_file)?)?;

        fs::rename(temp_path, target_path)?;
    }

    validate_deps()?;
    Ok(())
}

pub async fn build_workspace() -> std::io::Result<()> {
    let path = PathBuf::from(".");
    let mut ruborist = Ruborist::new(path.clone());
    ruborist.build_workspace_tree().await?;

    if let Some(ideal_tree) = &ruborist.ideal_tree {
        let mut node_list = Vec::new();
        let mut edges = Vec::new();
        let mut workspace_names = HashSet::new();

        for child in ideal_tree.children.read().unwrap().iter() {
            let name = child.name.clone();
            if child.is_link {
                continue;
            }
            workspace_names.insert(name.clone());
            node_list.push(json!({
                "name": name,
                "path": child.path.clone(),
            }));
        }

        for child in ideal_tree.children.read().unwrap().iter() {
            for edge in child.edges_out.read().unwrap().iter() {
                if *edge.valid.read().unwrap() {
                    if let Some(to_node) = edge.to.read().unwrap().as_ref() {
                        edges.push(json!([to_node.name.clone(), edge.from.name.clone()]));
                    }
                }
            }
        }

        let workspace_file = json!({
            "nodeList": node_list,
            "edges": edges,
        });

        let temp_path = path.join("workspace.json.tmp");
        let target_path = path.join("workspace.json");

        fs::write(&temp_path, serde_json::to_string_pretty(&workspace_file)?)?;
        fs::rename(temp_path, target_path)?;
    }

    Ok(())
}

fn validate_deps() -> std::io::Result<()> {
    let path = PathBuf::from(".");
    let lock_path = path.join("package-lock.json");

    let lock_content = fs::read_to_string(lock_path)?;
    let lock_file: serde_json::Value = serde_json::from_str(&lock_content)?;

    if let Some(packages) = lock_file.get("packages").and_then(|p| p.as_object()) {
        for (pkg_path, pkg_info) in packages {
            // 检查所有类型的依赖
            let dep_types = [
                ("dependencies", false),
                ("peerDependencies", true),
                ("optionalDependencies", true),
                ("devDependencies", false)
            ];

            for (dep_field, is_optional) in dep_types {
                if let Some(dependencies) = pkg_info.get(dep_field).and_then(|d| d.as_object()) {
                    for (dep_name, req_version) in dependencies {
                        let req_version_str = req_version.as_str().unwrap_or_default();
                        let version_req = match VersionReq::parse(req_version_str) {
                            Ok(req) => req,
                            Err(_) => continue, // 版本解析失败时跳过该依赖的验证
                        };

                        // 逐级向上查找依赖
                        let mut current_path = if pkg_path.is_empty() {
                            String::new()
                        } else {
                            pkg_path.to_string()
                        };
                        let mut dep_info = None;

                        // 从当前包所在目录开始，逐级向上查找
                        loop {
                            let search_path = if current_path.is_empty() {
                                format!("node_modules/{}", dep_name)
                            } else {
                                format!("{}/node_modules/{}", current_path, dep_name)
                            };

                            if let Some(info) = packages.get(&search_path) {
                                dep_info = Some(info);
                                current_path = search_path;
                                break;
                            }

                            // 如果已经到达顶层，则退出
                            if current_path.is_empty() {
                                break;
                            }

                            // 移除最后一个路径段，向上一级查找
                            if let Some(last_modules) = current_path.rfind("/node_modules/") {
                                current_path = current_path[..last_modules].to_string();
                            } else {
                                current_path = String::new();
                            }
                        }

                        // 对于可选依赖，如果找不到包则跳过
                        if let Some(dep_info) = dep_info {
                            if let Some(actual_version) = dep_info.get("version").and_then(|v| v.as_str()) {
                                let version = Version::parse(actual_version)
                                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                                if !version_req.matches(&version) {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        format!(
                                            "包 {} 中的{}依赖 {} (要求版本: {}) 与实际版本 {}@{} 不匹配",
                                            pkg_path, dep_field, dep_name, req_version_str, current_path, actual_version
                                        )
                                    ));
                                }
                            }
                        } else if !is_optional {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::NotFound,
                                format!("包 {} 中声明的{}依赖 {} 在依赖树中未找到",
                                    pkg_path, dep_field, dep_name)
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
