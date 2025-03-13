use semver::{Version, VersionReq};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use super::logger::{log_info, log_verbose};
use super::registry::resolve;

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeType {
    Prod,     // Production dependency
    Dev,      // Development dependency
    Peer,     // Peer dependency
    Optional, // Optional dependency
}

#[derive(Debug, Clone)]
pub struct OverrideRule {
    pub name: String,
    pub spec: String,
    pub target_spec: String,
    pub parent: Option<Box<OverrideRule>>,
}

#[derive(Debug)]
pub struct Overrides {
    pub package: Value,
    pub rules: Vec<OverrideRule>,
}

#[derive(Debug)]
pub struct Node {
    // Basic info (immutable)
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub package: Value,

    // Nested relationships (need mutable access)
    pub parent: RwLock<Option<Arc<Node>>>,
    pub children: RwLock<Vec<Arc<Node>>>,

    // Edge relationships (need mutable access)
    pub edges_out: RwLock<Vec<Arc<Edge>>>,
    pub edges_in: RwLock<Vec<Arc<Edge>>>,

    // Root node flag (immutable)
    pub is_root: bool,

    // Workspace/link status (immutable)
    pub is_workspace: bool,
    pub is_link: bool,
    pub target: RwLock<Option<Arc<Node>>>,

    // Installation configuration
    pub install_links: bool,

    // Dependency type flags (mutable)
    pub is_optional: RwLock<Option<bool>>,
    pub is_peer: RwLock<Option<bool>>,
    pub is_dev: RwLock<Option<bool>>,
    pub is_prod: RwLock<Option<bool>>,

    // Overrides configuration
    pub overrides: Option<Overrides>,
}

#[derive(Debug)]
pub struct Edge {
    // Basic info (immutable)
    pub name: String,
    pub spec: String,

    // Relationship info (immutable)
    pub from: Arc<Node>,
    pub to: RwLock<Option<Arc<Node>>>,

    // Resolution status
    pub valid: RwLock<bool>,

    // Edge type (immutable)
    pub edge_type: EdgeType,
}

impl Node {
    pub fn new(name: String, path: PathBuf, pkg: Value) -> Arc<Self> {
        Arc::new(Self {
            name,
            version: pkg["version"].as_str().unwrap_or("").to_string(),
            path,
            package: pkg,
            parent: RwLock::new(None),
            children: RwLock::new(Vec::new()),
            edges_out: RwLock::new(Vec::new()),
            edges_in: RwLock::new(Vec::new()),
            is_root: false,
            is_link: false,
            target: RwLock::new(None),
            is_workspace: false,
            install_links: false,
            is_dev: RwLock::new(None),
            is_peer: RwLock::new(None),
            is_optional: RwLock::new(None),
            is_prod: RwLock::new(None),
            overrides: None,
        })
    }

    pub fn new_root(name: String, path: PathBuf, pkg: Value) -> Arc<Self> {
        Arc::new(Self {
            name,
            version: pkg["version"].as_str().unwrap_or("").to_string(),
            path,
            package: pkg.clone(),
            parent: RwLock::new(None),
            children: RwLock::new(Vec::new()),
            edges_out: RwLock::new(Vec::new()),
            edges_in: RwLock::new(Vec::new()),
            is_root: true,
            is_link: false,
            target: RwLock::new(None),
            is_workspace: false,
            install_links: false,
            is_dev: RwLock::new(None),
            is_peer: RwLock::new(None),
            is_optional: RwLock::new(None),
            is_prod: RwLock::new(None),
            overrides: Overrides::new(pkg.clone()).parse(pkg.clone()),
        })
    }

    pub fn new_link(name: String, target: Arc<Node>) -> Arc<Self> {
        Arc::new(Self {
            name,
            is_link: true,
            path: target.path.clone(),
            package: target.package.clone(),
            version: target.version.clone(),
            target: RwLock::new(Some(target)),
            parent: RwLock::new(None),
            children: RwLock::new(Vec::new()),
            edges_out: RwLock::new(Vec::new()),
            edges_in: RwLock::new(Vec::new()),
            is_root: false,
            is_workspace: false,
            install_links: false,
            is_dev: RwLock::new(None),
            is_peer: RwLock::new(None),
            is_optional: RwLock::new(None),
            is_prod: RwLock::new(None),
            overrides: None,
        })
    }

    pub fn new_workspace(name: String, path: PathBuf, pkg: Value) -> Arc<Self> {
        Arc::new(Self {
            name,
            version: pkg["version"].as_str().unwrap_or("*").to_string(),
            path,
            package: pkg,
            parent: RwLock::new(None),
            children: RwLock::new(Vec::new()),
            edges_out: RwLock::new(Vec::new()),
            edges_in: RwLock::new(Vec::new()),
            is_root: false,
            is_workspace: true,
            is_link: false,
            target: RwLock::new(None),
            install_links: false,
            is_dev: RwLock::new(None),
            is_peer: RwLock::new(None),
            is_optional: RwLock::new(None),
            is_prod: RwLock::new(None),
            overrides: None,
        })
    }

    pub fn add_child(&self, child: Arc<Node>) {
        let mut children = self.children.write().unwrap();
        children.push(child);
    }

    pub async fn add_edge(&self, mut edge: Arc<Edge>) {
        // Find root node for override rules
        let mut current = Some(edge.from.clone());
        let mut root = None;

        while let Some(node) = current {
            if node.is_root {
                root = Some(node);
                break;
            }
            current = node.parent.read().unwrap().as_ref().cloned();
        }

        // Apply override rules if exists
        if let Some(root) = root {
            if let Some(overrides) = &root.overrides {
                'rules: for rule in &overrides.rules {
                    // Check name match
                    if edge.name != rule.name {
                        continue;
                    }

                    // Check version spec matching
                    if rule.spec != "*" {
                        let matches = if edge.spec == "*" {
                            true
                        } else {
                            // Handle version matching logic
                            match Version::parse(&rule.spec) {
                                Ok(rule_version) => match VersionReq::parse(&edge.spec) {
                                    Ok(edge_req) => edge_req.matches(&rule_version),
                                    _ => edge.spec == "*" || edge.spec == rule.spec,
                                },
                                _ => match VersionReq::parse(&rule.spec) {
                                    Ok(rule_req) => {
                                        if let Ok(resolved) = resolve(&edge.name, &edge.spec).await
                                        {
                                            if let Some(version) = resolved
                                                .manifest
                                                .get("version")
                                                .and_then(|v| v.as_str())
                                            {
                                                if let Ok(version) = Version::parse(version) {
                                                    rule_req.matches(&version)
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        }
                                    }
                                    _ => edge.spec == "*" || edge.spec == rule.spec,
                                },
                            }
                        };
                        if !matches {
                            continue;
                        }
                    }

                    // Check parent rule chain
                    if let Some(mut current_rule) = rule.parent.as_ref() {
                        let mut current_node = edge.from.parent.read().unwrap().clone();

                        'find_match: while let Some(node) = current_node {
                            if node.name == current_rule.name {
                                let matches = if current_rule.spec == "*" {
                                    true
                                } else {
                                    match (
                                        Version::parse(&node.version),
                                        VersionReq::parse(&current_rule.spec),
                                    ) {
                                        (Ok(version), Ok(req)) => req.matches(&version),
                                        _ => node.version == current_rule.spec,
                                    }
                                };

                                if matches {
                                    if let Some(next_rule) = current_rule.parent.as_ref() {
                                        current_rule = next_rule;
                                        current_node = node.parent.read().unwrap().clone();
                                        continue;
                                    } else {
                                        break;
                                    }
                                }
                            }
                            current_node = node.parent.read().unwrap().clone();
                        }

                        if current_rule.parent.is_some() {
                            continue 'rules;
                        }
                    }

                    // Apply override rule
                    if let Some(edge_mut) = Arc::get_mut(&mut edge) {
                        log_info(&format!(
                            "Override rule applied {}@{} => {}",
                            rule.name, rule.spec, rule.target_spec
                        ));
                        edge_mut.spec = rule.target_spec.clone();
                    }
                    break;
                }
            }
        }

        let mut edges = self.edges_out.write().unwrap();
        edges.push(edge);
    }

    // Add incoming edge reference
    pub fn add_invoke(&self, edge: &Arc<Edge>) {
        let mut edges = self.edges_in.write().unwrap();
        edges.push(edge.clone());
    }

    // Update node type based on incoming edges
    // Rules:
    // 1. If any edge.from is prod and edge type is Prod: mark as prod (others false)
    // 2. If all edges are optional: mark as optional
    // 3. If all edges are dev: mark as dev
    // 4. If all edges are peer: mark as peer
    // Propagate changes to outgoing edges if type changes
    pub fn update_type(&self) {
        if self.is_root {
            return;
        }

        let edges_in = self.edges_in.read().unwrap();
        if edges_in.is_empty() {
            return;
        }

        let mut has_prod = false;
        let mut all_optional = true;
        let mut all_dev = true;
        let mut all_peer = true;

        // Analyze incoming edges
        for edge in edges_in.iter() {
            let from_node = &edge.from;

            if *from_node.is_prod.read().unwrap() == Some(true) && edge.edge_type == EdgeType::Prod
            {
                has_prod = true;
                all_optional = false;
                all_dev = false;
                all_peer = false;
                break;
            }

            if *from_node.is_optional.read().unwrap() != Some(true)
                && edge.edge_type != EdgeType::Optional
            {
                all_optional = false;
            }
            if *from_node.is_dev.read().unwrap() != Some(true) && edge.edge_type != EdgeType::Dev {
                all_dev = false;
            }
            if *from_node.is_peer.read().unwrap() != Some(true) && edge.edge_type != EdgeType::Peer
            {
                all_peer = false;
            }
        }

        // Update node status
        let mut changed = false;

        if has_prod {
            if *self.is_prod.read().unwrap() != Some(true) {
                *self.is_prod.write().unwrap() = Some(true);
                *self.is_optional.write().unwrap() = Some(false);
                *self.is_dev.write().unwrap() = Some(false);
                *self.is_peer.write().unwrap() = Some(false);
                changed = true;
            }
        } else if all_optional {
            if *self.is_optional.read().unwrap() != Some(true) {
                *self.is_optional.write().unwrap() = Some(true);
                *self.is_prod.write().unwrap() = Some(false);
                changed = true;
            }
        } else if all_dev {
            if *self.is_dev.read().unwrap() != Some(true) {
                *self.is_dev.write().unwrap() = Some(true);
                *self.is_prod.write().unwrap() = Some(false);
                changed = true;
            }
        } else if all_peer {
            if *self.is_peer.read().unwrap() != Some(true) {
                *self.is_peer.write().unwrap() = Some(true);
                *self.is_prod.write().unwrap() = Some(false);
                changed = true;
            }
        }

        log_verbose(&format!(
            "{}@{} type changed [all_optional{}]",
            &self.name, &self.version, all_optional
        ));

        // Propagate changes
        if changed {
            let edges_out = self.edges_out.read().unwrap();
            for edge in edges_out.iter() {
                if let Some(to_node) = edge.to.read().unwrap().as_ref() {
                    to_node.update_type();
                }
            }
        }
    }
}

impl Edge {
    pub fn new(from: Arc<Node>, edge_type: EdgeType, name: String, spec: String) -> Arc<Self> {
        Arc::new(Self {
            name,
            spec: if spec.trim().is_empty() {
                "*".to_string()
            } else {
                spec
            },
            from,
            to: RwLock::new(None),
            valid: RwLock::new(false),
            edge_type,
        })
    }
}

impl Overrides {
    pub fn new(pkg: Value) -> Self {
        Self {
            package: pkg,
            rules: vec![],
        }
    }

    pub fn parse(&self, pkg: Value) -> Option<Self> {
        let overrides = pkg.get("overrides")?;
        let mut rules = Vec::new();
        self.parse_rules(overrides, None, &mut rules);
        Some(Self {
            package: pkg,
            rules,
        })
    }

    // Recursively parse override rules
    fn parse_rules(
        &self,
        value: &Value,
        parent: Option<Box<OverrideRule>>,
        rules: &mut Vec<OverrideRule>,
    ) {
        if let Some(obj) = value.as_object() {
            for (key, value) in obj {
                if key == "." {
                    // Handle current level override
                    if let Some(parent) = parent.as_ref() {
                        let mut new_parent = parent.clone();
                        new_parent.target_spec = self.parse_target_spec(value);
                        rules.push(*new_parent);
                    }
                    continue;
                }

                // Parse name@spec format
                let (name, spec) = Self::parse_name_spec(key);

                if value.is_object() {
                    // Nested rules with parent relationship
                    let parent_rule = Box::new(OverrideRule {
                        name: name.to_string(),
                        spec: spec.to_string(),
                        target_spec: String::from("*"),
                        parent: parent.clone(),
                    });

                    self.parse_rules(value, Some(parent_rule), rules);
                } else {
                    // Direct rule
                    rules.push(OverrideRule {
                        name: name.to_string(),
                        spec: spec.to_string(),
                        target_spec: self.parse_target_spec(value),
                        parent: parent.clone(),
                    });
                }
            }
        }
    }

    // Split name@spec format
    fn parse_name_spec(key: &str) -> (&str, &str) {
        key.rfind('@')
            .map(|idx| (&key[..idx], &key[idx + 1..]))
            .unwrap_or((key, "*"))
    }

    // Resolve target spec with reference syntax
    fn parse_target_spec(&self, value: &Value) -> String {
        match value {
            Value::String(s) if s.starts_with('$') => {
                let dep_name = &s[1..];
                self.package
                    .get("dependencies")
                    .and_then(|deps| deps.get(dep_name))
                    .or_else(|| {
                        self.package
                            .get("devDependencies")
                            .and_then(|d| d.get(dep_name))
                    })
                    .and_then(|v| v.as_str())
                    .unwrap_or("*")
                    .to_string()
            }
            Value::String(s) => s.clone(),
            _ => String::from("*"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_overrides() {
        // Test basic overrides parsing
        let pkg = json!({
            "name": "test-pkg",
            "version": "1.0.0",
            "overrides": {
                "a": "1.0.0",
                "b": {
                    ".": "2.0.0"
                },
                "c@1": "3.0.0"
            }
        });

        let overrides = Overrides::new(pkg.clone()).parse(pkg.clone()).unwrap();
        assert_eq!(overrides.rules.len(), 3);

        // Verify direct rules
        let a_rule = overrides.rules.iter().find(|r| r.name == "a").unwrap();
        assert_eq!(a_rule.spec, "*");
        assert_eq!(a_rule.target_spec, "1.0.0");

        // Verify nested rules
        let b_rule = overrides.rules.iter().find(|r| r.name == "b").unwrap();
        assert_eq!(b_rule.spec, "*");
        assert_eq!(b_rule.target_spec, "2.0.0");

        // Verify versioned rules
        let c_rule = overrides.rules.iter().find(|r| r.name == "c").unwrap();
        assert_eq!(c_rule.spec, "1");
        assert_eq!(c_rule.target_spec, "3.0.0");
    }

    #[test]
    fn test_parse_nested_version_overrides() {
        // Test nested version overrides
        let pkg = json!({
            "name": "test-pkg",
            "version": "1.0.0",
            "overrides": {
                "a@1.0.0": {
                    "b": "2.0.0"
                }
            }
        });

        let overrides = Overrides::new(pkg.clone()).parse(pkg.clone()).unwrap();
        let b_rule = overrides.rules.iter().find(|r| r.name == "b").unwrap();
        assert_eq!(b_rule.target_spec, "2.0.0");

        // Verify parent rule chain
        let parent = b_rule.parent.as_ref().unwrap();
        assert_eq!(parent.name, "a");
        assert_eq!(parent.spec, "1.0.0");
    }

    #[test]
    fn test_parse_empty_overrides() {
        // Test package without overrides
        let pkg = json!({
            "name": "test-pkg",
            "version": "1.0.0"
        });

        let overrides = Overrides::new(pkg.clone()).parse(pkg.clone());
        assert!(overrides.is_none());
    }

    #[test]
    fn test_parse_invalid_overrides() {
        // Test invalid overrides format
        let pkg = json!({
            "name": "test-pkg",
            "version": "1.0.0",
            "overrides": "invalid"
        });

        let overrides = Overrides::new(pkg.clone()).parse(pkg.clone());
        assert!(overrides.unwrap().rules.is_empty());
    }
}
