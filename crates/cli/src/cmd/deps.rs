use crate::helper::lock::{serialize_tree_to_packages, validate_deps, write_ideal_tree_to_lock_file};
use crate::helper::ruborist::Ruborist;
use crate::util::json::load_package_json;
use crate::util::logger::{log_info, log_verbose};
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

pub async fn build_deps() -> Result<()> {
    let path = PathBuf::from(".");
    let mut ruborist = Ruborist::new(path.clone());
    ruborist.build_ideal_tree().await?;

    let pkg_file = load_package_json()?;
    const MAX_RETRIES: u32 = 5;
    let mut retry_count = 0;

    loop {
        let pkgs_in_tree = {
            let to_guard = ruborist.ideal_tree.as_ref().unwrap();
            serialize_tree_to_packages(to_guard)
        };

        let invalid_deps = validate_deps(&pkg_file, &pkgs_in_tree).await?;

        if invalid_deps.is_empty() {
            log_verbose("No invalid dependencies found");
            break;
        }

        if retry_count >= MAX_RETRIES {
            return Err(anyhow::anyhow!("Failed to fix dependencies after {} retries", MAX_RETRIES));
        }

        for dep in invalid_deps {
            log_verbose(&format!(
                "Fixing dependency: {}/{}",
                dep.package_path, dep.dependency_name
            ));
            // Try to fix the dependency
            if let Err(e) = ruborist
                .fix_dep_path(&dep.package_path, &dep.dependency_name)
                .await
            {
                log_verbose(&format!("Failed to fix dependency: {}", e));
                return Err(anyhow::anyhow!("Failed to fix dependency: {}", e));
            } else {
                log_verbose(&format!("Fixed dependency: {}/{}", dep.package_path, dep.dependency_name));
            }
        }

        retry_count += 1;
    }

    let tree = ruborist.ideal_tree.unwrap();
    write_ideal_tree_to_lock_file(&tree).await?;

    Ok(())
}

pub async fn build_workspace() -> Result<()> {
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

        fs::write(&temp_path, serde_json::to_string_pretty(&workspace_file)?)
            .context("Failed to write temporary workspace.json")?;
        fs::rename(temp_path, target_path).context("Failed to rename temporary workspace.json")?;
    }

    Ok(())
}
