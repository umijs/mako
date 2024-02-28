use std::path::Path;

use mako_core::anyhow::Result;

use crate::resolve::ResolverResource;

pub enum TaskType {
    Entry(String),
    Normal(String),
}

#[derive(Debug, Clone)]
pub struct Task {
    // origin path, might includes query
    pub path: String,
    pub request: FileRequest,
    pub is_entry: bool,
    pub parent_resource: Option<ResolverResource>,
    pub ext_name: Option<String>,
}

impl Task {
    pub fn new(task_type: TaskType, parent_resource: Option<ResolverResource>) -> Self {
        let (path, is_entry) = match task_type {
            TaskType::Entry(path) => (path, true),
            TaskType::Normal(path) => (path, false),
        };
        let request = parse_path(&path).unwrap();
        let ext_name = ext_name(&request.path).map(|s| s.to_string());
        Self {
            path,
            parent_resource,
            is_entry,
            request,
            ext_name,
        }
    }

    pub fn from_normal_path(path: String) -> Self {
        let task_type = TaskType::Normal(path);
        Self::new(task_type, None)
    }

    pub fn is_match(&self, ext_names: Vec<&str>) -> bool {
        if let Some(ext_name) = &self.ext_name {
            ext_names.contains(&ext_name.as_str())
        } else {
            false
        }
    }
}

impl Default for Task {
    fn default() -> Self {
        Self {
            path: "test.js".to_string(),
            parent_resource: None,
            is_entry: false,
            request: FileRequest {
                path: "".to_string(),
                query: vec![],
            },
            ext_name: None,
        }
    }
}

pub fn parse_path(path: &str) -> Result<FileRequest> {
    let mut iter = path.split('?');
    let path = iter.next().unwrap();
    let query = iter.next().unwrap_or("");
    let mut query_vec = vec![];
    for pair in query.split('&') {
        if pair.contains('=') {
            let mut it = pair.split('=').take(2);
            let kv = match (it.next(), it.next()) {
                (Some(k), Some(v)) => (k.to_string(), v.to_string()),
                _ => continue,
            };
            query_vec.push(kv);
        } else if !pair.is_empty() {
            query_vec.push((pair.to_string(), "".to_string()));
        }
    }
    Ok(FileRequest {
        path: path.to_string(),
        query: query_vec,
    })
}

#[derive(Debug, Clone)]
pub struct FileRequest {
    pub path: String,
    pub query: Vec<(String, String)>,
}

impl FileRequest {
    pub fn has_query(&self, key: &str) -> bool {
        self.query.iter().any(|(k, _)| *k == key)
    }
}

pub fn ext_name(path: &str) -> Option<&str> {
    let path = Path::new(path);
    if let (true, Some(ext)) = (path.is_file(), path.extension()) {
        return ext.to_str();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_path;

    #[test]
    fn test_parse_path() {
        let result = parse_path("foo").unwrap();
        assert_eq!(result.path, "foo");
        assert_eq!(result.query, vec![]);

        let result = parse_path("foo?bar=1&hoo=2").unwrap();
        assert_eq!(result.path, "foo");
        assert_eq!(
            result.query.first().unwrap(),
            &("bar".to_string(), "1".to_string())
        );
        assert_eq!(
            result.query.get(1).unwrap(),
            &("hoo".to_string(), "2".to_string())
        );
        assert!(result.has_query("bar"));
        assert!(result.has_query("hoo"));
        assert!(!result.has_query("foo"));

        let result = parse_path("foo?bar").unwrap();
        assert_eq!(result.path, "foo");
        assert_eq!(
            result.query.first().unwrap(),
            &("bar".to_string(), "".to_string())
        );
        assert!(result.has_query("bar"));
    }
}
