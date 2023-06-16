use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use swc_common::sync::Lrc;
use swc_common::SourceMap;

use crate::chunk_graph::ChunkGraph;
use crate::config::Config;
use crate::module_graph::ModuleGraph;

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub config: Config,
    pub root: PathBuf,
    pub meta: Meta,
}

pub struct Meta {
    pub script: ScriptMeta,
    pub css: CssMeta,
}

impl Meta {
    pub fn new() -> Self {
        Self {
            script: ScriptMeta::new(),
            css: CssMeta::new(),
        }
    }
}

pub struct ScriptMeta {
    pub cm: Lrc<SourceMap>,
}

impl ScriptMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
        }
    }
}

pub struct CssMeta {
    pub cm: Lrc<SourceMap>,
}

impl CssMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
        }
    }
}

impl Context {
    pub fn emit_assets(&self, k: String, v: String) {
        let mut assets_info = self.assets_info.lock().unwrap();
        assets_info.insert(k, v);
        drop(assets_info);
    }
}

pub struct Compiler {
    pub context: Arc<Context>,
}

impl Compiler {
    pub fn new(config: Config, root: PathBuf) -> Self {
        assert!(root.is_absolute(), "root path must be absolute");
        Self {
            context: Arc::new(Context {
                config,
                root,
                module_graph: RwLock::new(ModuleGraph::new()),
                chunk_graph: RwLock::new(ChunkGraph::new()),
                assets_info: Mutex::new(HashMap::new()),
                meta: Meta::new(),
            }),
        }
    }

    pub fn compile(&self) {
        self.build();
        self.generate();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Compiler;
    use crate::config::Config;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_config_inline_limit() {
        let (files, file_contents) = compile("test/compile/config-inline-limit");
        println!("{:?}", files);
        assert_eq!(
            files.join(",").contains(&".jpg".to_string()),
            true,
            "big.jpg is not inlined"
        );
        assert_eq!(
            files.join(",").contains(&".png".to_string()),
            false,
            "small.png is inlined"
        );
        assert_eq!(files.len(), 3, "index.js, index.js.map, xxx.jpg");
        let index_js_content = file_contents.get("index.js").unwrap();
        assert_eq!(
            index_js_content.contains("data:image/png;base64,"),
            true,
            "small.png is inlined"
        );
    }

    fn compile(base: &str) -> (Vec<String>, HashMap<String, String>) {
        let current_dir = std::env::current_dir().unwrap();
        let root = current_dir.join(base);
        let config = Config::new(&root, None, None).unwrap();
        let compiler = Compiler::new(config, root.clone());
        compiler.compile();
        let dist = root.join("dist");
        let files = std::fs::read_dir(dist.clone())
            .unwrap()
            .map(|f| f.unwrap().path().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let mut file_contents = HashMap::new();
        let files = files
            .iter()
            .map(|f| f.replace(format!("{}/", dist.to_str().unwrap()).as_str(), ""))
            .collect::<Vec<_>>();
        for file in files.iter() {
            if file.ends_with(".js") || file.ends_with(".css") {
                let content = std::fs::read_to_string(dist.join(file)).unwrap();
                file_contents.insert(file.to_string(), content);
            }
        }
        (files, file_contents)
    }
}
