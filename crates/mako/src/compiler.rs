use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use colored::Colorize;
use swc_common::sync::Lrc;
use swc_common::{Globals, SourceMap, DUMMY_SP};
use swc_ecma_ast::Ident;

use crate::chunk_graph::ChunkGraph;
use crate::comments::Comments;
use crate::config::{Config, OutputMode};
use crate::module_graph::ModuleGraph;
use crate::plugin::{Plugin, PluginDriver};
use crate::plugins;
use crate::resolve::{get_resolvers, Resolvers};
use crate::stats::StatsInfo;

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub modules_with_missing_deps: RwLock<Vec<String>>,
    pub config: Config,
    pub args: Args,
    pub root: PathBuf,
    pub meta: Meta,
    pub plugin_driver: PluginDriver,
    pub stats_info: Mutex<StatsInfo>,
    pub resolvers: Resolvers,
}

#[derive(Default)]
pub struct Args {
    pub watch: bool,
}

impl Default for Context {
    fn default() -> Self {
        let config: Config = Default::default();
        let resolvers = get_resolvers(&config);

        Self {
            config: Default::default(),
            args: Args { watch: false },
            root: PathBuf::from(""),
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            modules_with_missing_deps: RwLock::new(Vec::new()),
            meta: Meta::new(),
            plugin_driver: Default::default(),
            // 产物信息放在上下文里是否合适
            stats_info: Mutex::new(StatsInfo::new()),
            resolvers,
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
    // These idents may be used in other places, such as transform_async_module
    pub module_ident: Ident,
    pub exports_ident: Ident,
    pub require_ident: Ident,
}

impl ScriptMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
            origin_comments: Default::default(),
            output_comments: Default::default(),
            globals: Globals::default(),
            module_ident: build_ident("module"),
            exports_ident: build_ident("exports"),
            require_ident: build_ident("require"),
        }
    }
}

fn build_ident(ident: &str) -> Ident {
    Ident {
        span: DUMMY_SP,
        sym: ident.into(),
        optional: false,
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
    pub fn new(config: Config, root: PathBuf, args: Args) -> Self {
        assert!(root.is_absolute(), "root path must be absolute");

        let mut plugins: Vec<Arc<dyn Plugin>> = vec![
            // features
            Arc::new(plugins::node_polyfill::NodePolyfillPlugin {}),
            Arc::new(plugins::manifest::ManifestPlugin {}),
            Arc::new(plugins::copy::CopyPlugin {}),
            // file types
            Arc::new(plugins::css::CSSPlugin {}),
            Arc::new(plugins::less::LessPlugin {}),
            Arc::new(plugins::javascript::JavaScriptPlugin {}),
            Arc::new(plugins::json::JSONPlugin {}),
            Arc::new(plugins::md::MdPlugin {}),
            Arc::new(plugins::svg::SVGPlugin {}),
            Arc::new(plugins::toml::TOMLPlugin {}),
            Arc::new(plugins::wasm::WASMPlugin {}),
            Arc::new(plugins::xml::XMLPlugin {}),
            Arc::new(plugins::yaml::YAMLPlugin {}),
            Arc::new(plugins::assets::AssetsPlugin {}),
            Arc::new(plugins::runtime::MakoRuntime {}),
            Arc::new(plugins::farm_tree_shake::FarmTreeShake {}),
        ];

        let mut config = config;

        if config.output.mode == OutputMode::MinifishPrebuild {
            plugins.insert(
                0,
                Arc::new(plugins::minifish_compiler::MinifishCompiler::new(
                    &config, &root,
                )),
            );
            plugins.push(Arc::new(
                plugins::minifish_analyze_deps::MinifishDepsAnalyze {},
            ));
        }

        let plugin_driver = PluginDriver::new(plugins);

        plugin_driver.modify_config(&mut config).unwrap();

        let resolvers = get_resolvers(&config);
        Self {
            context: Arc::new(Context {
                config,
                args,
                root,
                module_graph: RwLock::new(ModuleGraph::new()),
                chunk_graph: RwLock::new(ChunkGraph::new()),
                assets_info: Mutex::new(HashMap::new()),
                modules_with_missing_deps: RwLock::new(Vec::new()),
                meta: Meta::new(),
                plugin_driver,
                stats_info: Mutex::new(StatsInfo::new()),
                resolvers,
            }),
        }
    }

    pub fn compile(&self) {
        // 先清空 dist 目录
        self.clean_dist();

        let t_compiler = Instant::now();
        let is_prod = self.context.config.mode == crate::config::Mode::Production;
        let building_with_message = format!(
            "Building with {} for {}...",
            "mako".to_string().cyan(),
            if is_prod { "production" } else { "development" }
        )
        .green();
        println!("{}", building_with_message);
        self.build();
        let result = self.generate();
        let t_compiler = t_compiler.elapsed();
        match result {
            Ok(_) => {
                println!(
                    "{}",
                    format!(
                        "✓ Built in {}",
                        format!("{}ms", t_compiler.as_millis()).bold()
                    )
                    .green()
                );
                if !self.context.args.watch {
                    println!("{}", "Complete!".bold());
                }
            }
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

    pub fn clean_dist(&self) {
        // compiler 前清除 dist，如果后续 dev 环境不在 output_path 里，需要再补上 dev 的逻辑
        let output_path = &self.context.config.output.path;
        if fs::metadata(output_path).is_ok() {
            fs::remove_dir_all(output_path).unwrap();
        }
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
        assert!(files.len() == 3, "index.js, index.js.map, xxx.jpg");
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            index_js_content.contains("data:image/png;base64,"),
            "small.png is inlined"
        );
    }

    // TODO: enable this case when support inline css
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn test_css_deps() {
        let (files, file_contents) = compile("test/compile/css-deps");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            index_js_content.contains("require(\"./foo.css\")"),
            "should require deps"
        );
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            index_js_content.contains("require(\"./bar.css\")"),
            "should handle none-relative path as relative deps"
        );
        assert!(
            index_js_content.contains("let css = `@import \"http://should-not-be-removed\";"),
            "should keep remote imports"
        );
        assert!(
            index_js_content.contains("background: url(\"data:image/png;base64,"),
            "should handle none-relative path as relative deps for background url"
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
        let index_css_content = file_contents.get("index.css").unwrap();
        assert!(
            index_css_content.contains("data:image/png;base64,"),
            "small.png is inlined"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_issue_247() {
        let (files, file_contents) = compile("test/compile/css-issue-247");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            !index_js_content.contains("umi.png"),
            "css only assets should be removed"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_modules() {
        let (files, file_contents) = compile("test/compile/css-modules");
        println!("{:?}", files);
        let index_css_content = file_contents.get("index.css").unwrap();
        assert!(index_css_content.contains(".foo-"), ".foo is css moduled");
        assert!(
            index_css_content.contains(".bar {"),
            ".bar with :global is not css moduled"
        );
        assert!(
            index_css_content.contains(".e {"),
            ".e with :global is not css moduled"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_nesting() {
        let (files, file_contents) = compile("test/compile/css-nesting");
        println!("{:?}", files);
        let index_css_content = file_contents.get("index.css").unwrap();
        assert!(
            index_css_content.contains(".foo .bar {"),
            "css nesting works"
        );
        assert!(
            index_css_content.contains(".hoo {"),
            "css nesting with :global works"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_prefixer() {
        let (files, file_contents) = compile("test/compile/css-prefixer");
        println!("{:?}", files);
        let index_css_content = file_contents.get("index.css").unwrap();
        assert!(
            index_css_content.contains("display: -ms-flexbox;"),
            "ie 10 prefixer"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_px2rem() {
        let (files, file_contents) = compile("test/compile/css-px2rem");
        println!("{:?}", files);
        let index_css_content = file_contents.get("index.css").unwrap();
        assert!(
            index_css_content.contains("margin: 0 0 20px;"),
            "prop_black_list should works"
        );
        assert!(index_css_content.contains("font-size: 0.32rem;"), "normal");
        assert!(
            index_css_content.contains("@media (min-width: 5rem) {"),
            "media query should be transformed"
        );
        assert!(
            index_css_content.contains("content: \"16px\";"),
            "content string should not be transformed"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_es_decorator() {
        let (files, file_contents) = compile("test/compile/es-decorator");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            index_js_content.contains("Foo = (0, _ts_decorate._)(["),
            "legacy decorator"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_load() {
        let (files, file_contents) = compile("test/compile/load");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();
        let index_css_content = file_contents.get("index.css").unwrap();
        assert!(
            index_js_content.contains("\"foo\": \"json\""),
            "json loader"
        );
        assert!(
            index_js_content.contains("var _default = MDXContent;"),
            "md loader"
        );
        assert!(
            index_js_content.contains("\"foo\": \"json5\""),
            "json5 loader"
        );
        assert!(
            index_js_content.contains("\"foo\": \"toml\""),
            "toml loader"
        );
        assert!(
            index_js_content.contains("\"$value\": \"foo\""),
            "xml loader"
        );
        assert!(
            index_js_content.contains("\"foo\": \"yaml\""),
            "yaml loader"
        );
        assert!(
            index_css_content.contains(".foo {\n  color: red;\n}"),
            "css loader"
        );
        assert!(index_css_content.contains(".jpg\");\n}"), "big.jpg in css");
        assert!(
            index_css_content.contains(".big {\n  background: url(\""),
            "small.png in css"
        );
        assert!(
            index_js_content.contains("big.jpg\": function("),
            "include big.jpg in js"
        );
        assert!(
            index_js_content.contains("small.png\": function("),
            "include small.png in js"
        );
        // TODO: svg
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_issue_311_single_dep_with_multiple_sources() {
        let (files, file_contents) =
            compile("test/compile/issue-311-single-dep-with-multiple-sources");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();
        assert!(
            !index_js_content.contains("require('./axios/foo');"),
            "should replace single dep with multiple sources"
        );
    }

    fn compile(base: &str) -> (Vec<String>, HashMap<String, String>) {
        let current_dir = std::env::current_dir().unwrap();
        let root = current_dir.join(base);
        let config = Config::new(&root, None, None).unwrap();
        let compiler = Compiler::new(config, root.clone(), Default::default());
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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_merge_in_css() {
        let (files, file_contents) = compile("test/compile/css-merge-in-css");
        println!("{:?}", files);
        let index_css_content = file_contents.get("index.css").unwrap();

        assert_eq!(
            index_css_content,
            r#".a {
  color: red;
}
.c {
  color: green;
}
.b {
  color: blue;
}
/*# sourceMappingURL=index.css.map*/"#,
            "css merge in css works"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_merge_in_js() {
        let (files, file_contents) = compile("test/compile/css-merge-in-js");
        println!("{:?}", files);
        let index_css_content = file_contents.get("index.css").unwrap();

        assert_eq!(
            index_css_content,
            r#".a {
  color: red;
}
.b {
  color: blue;
}
.c {
  color: green;
}
/*# sourceMappingURL=index.css.map*/"#,
            "css merge in js works"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_merge_mixed() {
        let (files, file_contents) = compile("test/compile/css-merge-mixed");
        println!("{:?}", files);
        let index_css_content = file_contents.get("index.css").unwrap();

        assert_eq!(
            index_css_content,
            r#".a {
  color: red;
}
.c {
  color: green;
}
.b {
  color: blue;
}
/*# sourceMappingURL=index.css.map*/"#,
            "css merge mixed works"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_css_async_chunk() {
        let (files, file_contents) = compile("test/compile/css-async-chunk");
        println!("{:?}", files);
        let index_js_content = file_contents.get("index.js").unwrap();

        assert!(
            index_js_content.contains("cssChunksIdToUrlMap[\"a.ts\"]"),
            "css async chunk works"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_auto_code_splitting() {
        let (files, file_contents) = compile("test/compile/auto-code-splitting");
        println!("{:?}", files);

        assert!(
            !files.contains(&"should-be-merge_ts-async.js".to_string()),
            "minimal async chunk should be merged"
        );

        assert!(
            files.contains(&"vendors-async.js".to_string()),
            "entry dependencies should be split to vendors"
        );

        assert!(
            files.contains(&"vendors_dynamic_0-async.js".to_string())
                && files.contains(&"vendors_dynamic_1-async.js".to_string()),
            "big vendors should be split again"
        );

        assert!(
            file_contents["index.js"].contains("\"context.ts\":")
              && !file_contents["should-be-split_ts-async.js"].contains("\"context.ts\":"),
            "async chunk should reuse modules that already merged into entry with another minimal async chunk"
        );

        assert!(
            files.contains(&"common_dynamic-async.js".to_string()),
            "common async modules should be split"
        );
    }
}
