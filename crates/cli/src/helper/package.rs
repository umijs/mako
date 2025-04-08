use crate::util::{
    logger::{log_info, log_warning},
    node::Node,
};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

pub fn parse_package_name(path: &str) -> (Option<String>, String, String) {
    let parts: Vec<&str> = path.split('/').collect();
    let len = parts.len();

    if len >= 2 {
        let last = parts[len - 1];
        let second_last = parts[len - 2];

        if second_last.starts_with('@') {
            // scoped package: @scope/name
            (
                Some(second_last.to_string()),
                last.to_string(),
                format!("{}/{}", second_last, last),
            )
        } else {
            // normal package
            (None, last.to_string(), last.to_string())
        }
    } else if len == 1 {
        // name only
        (None, parts[0].to_string(), parts[0].to_string())
    } else {
        // invalid path
        (None, path.to_string(), path.to_string())
    }
}

pub fn serialize_tree_to_packages(node: &Arc<Node>) -> Value {
    let mut packages = json!({});
    let mut stack = vec![(node.clone(), String::new())];
    let mut total_packages = 0;

    while let Some((current, prefix)) = stack.pop() {
        let children = current.children.read().unwrap();
        let mut name_count = HashMap::new();
        for child in children.iter() {
            if !child.is_link {
                *name_count.entry(child.name.as_str()).or_insert(0) += 1;
            }
        }
        for (name, count) in name_count {
            if count > 1 {
                log_warning(&format!(
                    "Found {} duplicate dependencies named '{}' under '{}'",
                    count, name, current.name
                ));
            }
        }
        let mut pkg_info = if current.is_root {
            json!({
                "name": current.name,
                "version": current.version,
            })
        } else {
            let mut info = json!({
                "name": current.package.get("name"),
            });

            if current.is_workspace {
            } else if current.is_link {
                // update resolved field
                info["link"] = json!(true);
                // resolvd => targetNode#path
                info["resolved"] = json!(current
                    .target
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .path
                    .to_string_lossy());
            } else {
                info["version"] = json!(current.package.get("version"));
                info["resolved"] = json!(current
                    .package
                    .get("dist")
                    .unwrap_or(&json!(""))
                    .get("tarball"));
                info["integrity"] = json!(current
                    .package
                    .get("dist")
                    .unwrap_or(&json!(""))
                    .get("integrity"));
                total_packages = total_packages + 1;
            }

            if *current.is_peer.read().unwrap() == Some(true) {
                info["peer"] = json!(true);
            }

            let is_dev = *current.is_dev.read().unwrap() == Some(true);
            let is_optional = *current.is_optional.read().unwrap() == Some(true);

            if is_dev && is_optional {
                info["devOptional"] = json!(true);
            } else if is_dev {
                info["dev"] = json!(true);
            } else if is_optional {
                info["optional"] = json!(true);
            }

            // hasBin
            if current.package.get("hasInstallScript") == Some(&json!(true)) {
                info["hasInstallScript"] = json!(true);
            }

            info
        };

        // add dependencies field
        let fields = if current.is_link {
            vec![]
        } else if current.is_root || current.is_workspace {
            vec![
                "dependencies",
                "devDependencies",
                "peerDependencies",
                "optionalDependencies",
            ]
        } else {
            vec![
                "dependencies",
                "peerDependencies",
                "optionalDependencies",
                "bin",
                "license",
                "engines",
                "os",
                "cpu",
            ]
        };

        for field in fields.iter() {
            if let Some(deps) = current.package.get(field) {
                if deps.is_object() {
                    if !deps.as_object().unwrap().is_empty() {
                        pkg_info[field] = deps.clone();
                    }
                } else {
                    // compatible for string type
                    pkg_info[field] = deps.clone();
                }
            }
        }

        // use "" for root node
        let key = if prefix.is_empty() {
            "".to_string()
        } else {
            prefix.clone()
        };
        packages[key] = pkg_info;

        // process children
        let children = current.children.read().unwrap();

        for child in children.iter() {
            let child_prefix = if prefix.is_empty() {
                if child.is_workspace {
                    format!("{}", child.path.to_string_lossy())
                } else {
                    format!("node_modules/{}", child.name)
                }
            } else {
                format!("{}/node_modules/{}", &prefix, child.name)
            };
            stack.push((child.clone(), child_prefix));
        }
    }

    log_info(&format!(
        "Total {} dependencies after merging",
        total_packages
    ));
    packages
}
