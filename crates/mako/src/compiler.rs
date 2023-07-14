use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use swc_common::sync::Lrc;
use swc_common::{Globals, SourceMap};

use crate::chunk_graph::ChunkGraph;
use crate::comments::Comments;
use crate::config::Config;
use crate::module_graph::ModuleGraph;
use crate::plugin::PluginDriver;
use crate::plugins;

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub config: Config,
    pub root: PathBuf,
    pub meta: Meta,
    pub plugin_driver: PluginDriver,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            config: Default::default(),
            root: PathBuf::from(""),
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            meta: Meta::new(),
            plugin_driver: Default::default(),
        }
    }
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

impl Default for Meta {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ScriptMeta {
    pub cm: Lrc<SourceMap>,
    pub origin_comments: RwLock<Comments>,
    pub output_comments: RwLock<Comments>,
    pub globals: Globals,
}

impl ScriptMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
            origin_comments: Default::default(),
            output_comments: Default::default(),
            globals: Globals::default(),
        }
    }
}

pub struct CssMeta {
    pub cm: Lrc<SourceMap>,
    pub globals: Globals,
}

impl CssMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
            globals: Globals::default(),
        }
    }
}

impl Context {
    pub fn emit_assets(&self, origin_path: String, output_path: String) {
        let mut assets_info = self.assets_info.lock().unwrap();
        assets_info.insert(origin_path, output_path);
        drop(assets_info);
    }
}

pub struct Compiler {
    pub context: Arc<Context>,
}

impl Compiler {
    pub fn new(config: Config, root: PathBuf) -> Self {
        assert!(root.is_absolute(), "root path must be absolute");

        let plugin_driver = PluginDriver::new(vec![
            // features
            Arc::new(plugins::node_polyfill::NodePolyfillPlugin {}),
            // file types
            Arc::new(plugins::css::CSSPlugin {}),
            Arc::new(plugins::javascript::JavaScriptPlugin {}),
            Arc::new(plugins::json::JSONPlugin {}),
            Arc::new(plugins::svg::SVGPlugin {}),
            Arc::new(plugins::toml::TOMLPlugin {}),
            Arc::new(plugins::wasm::WASMPlugin {}),
            Arc::new(plugins::xml::XMLPlugin {}),
            Arc::new(plugins::yaml::YAMLPlugin {}),
            Arc::new(plugins::assets::AssetsPlugin {}),
        ]);
        let mut config = config;
        plugin_driver.modify_config(&mut config).unwrap();

        Self {
            context: Arc::new(Context {
                config,
                root,
                module_graph: RwLock::new(ModuleGraph::new()),
                chunk_graph: RwLock::new(ChunkGraph::new()),
                assets_info: Mutex::new(HashMap::new()),
                meta: Meta::new(),
                plugin_driver,
            }),
        }
    }

    pub fn compile(&self) {
        self.build();
        let result = self.generate();
        match result {
            Ok(_) => {}
            Err(e) => {
                panic!("generate failed: {:?}", e);
            }
        }
    }

    pub fn full_hash(&self) -> u64 {
        let cg = self.context.chunk_graph.read().unwrap();
        let mg = self.context.module_graph.read().unwrap();
        cg.full_hash(&mg)
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
        assert!(
            files.join(",").contains(&".jpg".to_string()),
            "big.jpg is not inlined"
        );
        assert!(
            !files.join(",").contains(&".png".to_string()),
            "small.png is inlined"
        );
        assert_eq!(files.len(), 3, "index.js, index.js.map, xxx.jpg");
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            index_js_content.contains("data:image/png;base64,"),
            "small.png is inlined"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_inline_limit() {
        let (files, file_contents) = compile("test/compile/css-inline-limit");
        println!("{:?}", files);
        assert!(
            files.join(",").contains(&".jpg".to_string()),
            "big.jpg is not inlined"
        );
        assert!(
            !files.join(",").contains(&".png".to_string()),
            "small.png is inlined"
        );
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            index_js_content.contains("data:image/png;base64,"),
            "small.png is inlined"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_modules() {
        let (files, file_contents) = compile("test/compile/css-modules");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(index_js_content.contains(".foo-"), ".foo is css moduled");
        assert!(
            index_js_content.contains(".bar {"),
            ".bar with :global is not css moduled"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_nesting() {
        let (files, file_contents) = compile("test/compile/css-nesting");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            index_js_content.contains(".foo .bar {"),
            "css nesting works"
        );
        assert!(
            index_js_content.contains(".hoo {"),
            "css nesting with :global works"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_prefixer() {
        let (files, file_contents) = compile("test/compile/css-prefixer");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            index_js_content.contains("display: -ms-flexbox;"),
            "ie 10 prefixer"
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
