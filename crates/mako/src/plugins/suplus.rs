use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dashmap::DashSet;
use mako_core::anyhow::Result;
use mako_core::regex::Regex;

use crate::ast::file::{Content, File};
use crate::compiler::{Args, Compiler, Context};
use crate::config::{
    CodeSplittingStrategy, Config, OptimizeAllowChunks, OptimizeChunkGroup, OptimizeChunkOptions,
};
use crate::plugin::{NextBuildParam, Plugin, PluginLoadParam};

pub struct SUPlus {
    scanning: Arc<Mutex<bool>>,
    dependence_node_module_files: DashSet<File>,
    reversed_required_files: DashSet<File>,
}

enum CodeType {
    SourceCode,
    Dependency,
}

impl From<bool> for CodeType {
    fn from(value: bool) -> Self {
        if value {
            CodeType::Dependency
        } else {
            CodeType::SourceCode
        }
    }
}

impl SUPlus {
    pub fn new() -> Self {
        SUPlus {
            scanning: Arc::new(Mutex::new(true)),
            dependence_node_module_files: DashSet::new(),
            reversed_required_files: DashSet::new(),
        }
    }
}

impl Plugin for SUPlus {
    fn name(&self) -> &str {
        "speedup_plus"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if param.file.path.starts_with("virtual:E:") {
            let path_string = param.file.path.to_string_lossy().to_string();

            let path = PathBuf::from(path_string.as_str()[10..].to_string());

            return Ok(Some(Content::Js(format!(
                r#"
chunksIdToUrlMap.cached = "cachedcached.js";
__mako_require__.ensure("cached").then(()=>{{
__mako_require__("{}");
}}, console.log);
"#,
                path.to_string_lossy()
            ))));
        }
        Ok(None)
    }

    fn modify_config(&self, config: &mut Config, _root: &Path, _args: &Args) -> Result<()> {
        for p in config.entry.values_mut() {
            *p = PathBuf::from(format!("virtual:E:{}", p.to_string_lossy()));
        }

        config.code_splitting = Some(CodeSplittingStrategy::Advanced(OptimizeChunkOptions {
            min_size: 0,
            groups: vec![
                OptimizeChunkGroup {
                    name: "cached".to_string(),
                    allow_chunks: OptimizeAllowChunks::All,
                    min_chunks: 0,
                    min_size: 0,
                    max_size: usize::MAX,
                    priority: 0,
                    test: Regex::new(r"[/\\]node_modules[/\\]").ok(),
                },
                OptimizeChunkGroup {
                    name: "common".to_string(),
                    min_chunks: 0,
                    // always split, to avoid multi-instance risk
                    min_size: 1,
                    max_size: usize::MAX,
                    priority: 99,
                    ..Default::default()
                },
            ],
        }));

        Ok(())
    }

    fn next_build(&self, _next_build_param: &NextBuildParam) -> bool {
        let from: CodeType = _next_build_param
            .current_module
            .id
            .contains("node_modules")
            .into();
        let to = _next_build_param.next_file.is_under_node_modules.into();

        match (from, to) {
            (CodeType::SourceCode, CodeType::Dependency) => {
                self.dependence_node_module_files
                    .insert(_next_build_param.next_file.clone());
                let scanning = *self.scanning.lock().unwrap();
                !scanning
            }
            (CodeType::Dependency, CodeType::SourceCode) => {
                self.reversed_required_files
                    .insert(_next_build_param.next_file.clone());
                true
            }
            _ => true,
        }
    }

    fn after_build(&self, _context: &Arc<Context>, compiler: &Compiler) -> Result<()> {
        if std::env::var("CACHED").map_or(false, |val| val == "yes") {
            println!("skip dep build");
            return Ok(());
        }

        let files = self
            .dependence_node_module_files
            .iter()
            .map(|f| f.clone())
            .collect::<Vec<File>>();

        let mut s = self.scanning.lock().unwrap();
        *s = false;
        drop(s);

        println!("build dep");
        compiler.build(files)?;

        let mut s = self.scanning.lock().unwrap();
        *s = true;

        self.reversed_required_files
            .iter()
            .for_each(|f| println!("r: {:?}", f.path));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let re = Regex::new(r"^(?!.*node_modules).*").unwrap();
        let test_str1 = "example_path/node_modules/example";
        let test_str2 = "example_path/src/example";

        assert!(!re.is_match(test_str1)); // 匹配失败
        assert!(re.is_match(test_str2)); // 匹配成功
    }
}
