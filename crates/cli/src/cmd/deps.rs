use crate::helper::lock::validate_deps;
use crate::helper::{package::serialize_tree_to_packages, ruborist::Ruborist};
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;


pub async fn build_deps() -> Result<()> {
    let path = PathBuf::from(".");
    let mut ruborist = Ruborist::new(path.clone());
    ruborist.build_ideal_tree().await?;

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

        fs::write(&temp_path, serde_json::to_string_pretty(&lock_file)?)
            .context("Failed to write temporary package-lock.json")?;

        fs::rename(temp_path, target_path)
            .context("Failed to rename temporary package-lock.json")?;
    }

    validate_deps().await?;
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
