use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};

use crate::ast::file::Content;
use crate::compiler::Context;
use crate::plugin::{Plugin, PluginLoadParam};

pub struct CaseSensitivePlugin {
    cache_map: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl CaseSensitivePlugin {
    pub fn new() -> Self {
        CaseSensitivePlugin {
            cache_map: Default::default(),
        }
    }

    pub fn is_checkable(&self, load_param: &PluginLoadParam, root: &String) -> bool {
        let file_path = &load_param.file.path;
        if !load_param.file.path.starts_with(root) {
            return false;
        }
        for component in file_path.iter() {
            if component.eq_ignore_ascii_case("node_modules") {
                return false;
            }
        }
        true
    }

    pub fn check_case_sensitive(&self, file: &Path, root: &str) -> String {
        // 可变变量，在循环内会被修改
        let mut file_path = file.to_path_buf();
        let mut case_name = String::new();
        // 缓存map，file path做为key存在对应路径下的文件名和文件夹名
        let mut cache_map = self.cache_map.lock().unwrap_or_else(|e| e.into_inner());
        while file_path.to_string_lossy().len() >= root.len() {
            if let Some(current) = file_path.file_name() {
                let current_str = current.to_string_lossy().to_string();
                file_path.pop(); // parent directory
                let mut entries: Vec<String> = Vec::new();
                if let Some(dir) = file_path.to_str() {
                    if let Some(i) = cache_map.get(dir as &str) {
                        entries = i.to_vec();
                    } else if let Ok(files) = fs::read_dir(dir) {
                        files.for_each(|entry| {
                            entries.push(entry.unwrap().file_name().to_string_lossy().to_string());
                        });
                        cache_map.insert(dir.to_string(), entries.to_vec());
                    }
                }
                if !entries.contains(&current_str) {
                    if let Some(correct_name) = entries
                        .iter()
                        .find(|&x| x.to_lowercase() == current_str.to_lowercase())
                    {
                        case_name = correct_name.clone();
                        break;
                    }
                }
            }
        }
        case_name
    }
}

impl Plugin for CaseSensitivePlugin {
    fn name(&self) -> &str {
        "case_sensitive_plugin"
    }

    fn load(
        &self,
        load_param: &PluginLoadParam,
        context: &Arc<Context>,
    ) -> Result<Option<Content>> {
        let root = &context.root.to_string_lossy().to_string();
        if self.is_checkable(load_param, root) {
            let dist_path = self.check_case_sensitive(load_param.file.path.as_path(), root);
            if !dist_path.is_empty() {
                return Err(anyhow!(
                    "{} does not match the corresponding path on disk [{}]",
                    load_param.file.path.to_string_lossy().to_string(),
                    dist_path
                ));
            }
        }
        Ok(None)
    }
}
