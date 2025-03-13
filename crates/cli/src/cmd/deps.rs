use crate::helper::{package::serialize_tree_to_packages, ruborist::Ruborist};
use serde_json::json;
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

    Ok(())
}
