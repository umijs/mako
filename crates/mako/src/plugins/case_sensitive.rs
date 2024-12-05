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

    pub fn is_checkable(&self, _param: &PluginLoadParam, root: &String) -> bool {
        let file_path = &_param.file.path;
        if !_param.file.path.starts_with(root) {
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
                    } else {
                        match fs::read_dir(dir) {
                            Ok(files) => {
                                files.for_each(|entry| {
                                    entries.push(
                                        entry.unwrap().file_name().to_string_lossy().to_string(),
                                    );
                                });
                                cache_map.insert(dir.to_string(), entries.to_vec());
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                }
                if !entries.contains(&current_str) {
                    if let Some(correct_name) = entries
                        .iter()
                        .find(|&x| x.to_lowercase() == current_str.to_lowercase())
                    {
                        case_name = correct_name.clone();
                        println!(
                            "File name is case-insensitive. Correct name is: {}",
                            correct_name
                        );
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

    fn load(&self, _param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        let root = &_context.root.to_string_lossy().to_string();
        if self.is_checkable(_param, root) {
            let dist_path = self.check_case_sensitive(_param.file.path.as_path(), root);
            if !dist_path.is_empty() {
                return Err(anyhow!(
                    "{} does not match the corresponding path on disk [{}]",
                    _param.file.path.to_string_lossy().to_string(),
                    dist_path
                ));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::ast::file::File;
    use crate::plugin::Plugin;
    use crate::plugins::case_sensitive::{CaseSensitivePlugin, PluginLoadParam};
    use crate::utils::test_helper::setup_compiler;

    #[test]
    fn test_case_sensitive_checker() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/build/case-sensitive");
        let compiler = setup_compiler("test/build/case-sensitive", false);
        let plugin = CaseSensitivePlugin::new();
        let file = &File::new(
            root.join("Assets/umi-logo.png")
                .to_string_lossy()
                .to_string(),
            compiler.context.clone(),
        );
        let result = plugin.load(&PluginLoadParam { file }, &compiler.context);
        assert!(result.is_err());
    }
}
