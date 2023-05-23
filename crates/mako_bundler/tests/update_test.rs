use std::fs;

use mako_bundler::{
    assert_debug_snapshot, assert_display_snapshot,
    build::{
        build::BuildParam,
        update::{UpdateResult, UpdateType},
    },
    compiler::Compiler,
    config::Config,
    plugin::plugin_driver::PluginDriver,
};
use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::test(flavor = "multi_thread")]
async fn update() {
    let mut compiler = setup_compiler("update".into());
    setup_files(
        &compiler,
        vec![
            (
                "entry.js".into(),
                r#"
(async () => {
    await import('./chunk-1.js');
})();
"#
                .into(),
            ),
            (
                "chunk-1.js".into(),
                r#"
export default async function () {
    console.log(123);
}
"#
                .into(),
            ),
        ],
    );
    test_build(&mut compiler);
    {
        let module_graph = compiler.context.module_graph.read().unwrap();
        assert_display_snapshot!(&module_graph);
    }
    // 模拟文件更新，多一个文件少一个文件
    setup_files(
        &compiler,
        vec![
            (
                "entry.js".into(),
                r#"
(async () => {
    await import('./chunk-2.js');
})();
"#
                .into(),
            ),
            (
                "chunk-2.js".into(),
                r#"
export const foo = 1;
"#
                .into(),
            ),
        ],
    );
    let result = test_update(&compiler, vec![("entry.js".into(), UpdateType::Modify)]);

    {
        let module_graph = compiler.context.module_graph.read().unwrap();
        assert_display_snapshot!(&module_graph);
    }

    assert_debug_snapshot!(&result);
}

#[allow(clippy::useless_format)]
fn setup_compiler(name: String) -> Compiler {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("mako_bundler=debug")),
        )
        .without_time()
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    let cwd = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures")
        .join(&name)
        .to_string_lossy()
        .to_string();
    let mut config = Config::from_literal_str(
        format!(
            r#"
{{
    "entry": {{
        "entry": "entry.js"
    }},
    "root": "{}",
    "externals":  {{ "test": "test" }}
}}
            "#,
            cwd,
        )
        .as_str(),
    )
    .unwrap();
    config.normalize();

    Compiler::new(config, PluginDriver::new())
}

fn test_build(compiler: &mut Compiler) {
    compiler.build(&BuildParam { files: None }).unwrap();
}

fn test_update(compiler: &Compiler, paths: Vec<(String, UpdateType)>) -> UpdateResult {
    compiler.update(paths).unwrap()
}

fn setup_files(compiler: &Compiler, extra_files: Vec<(String, String)>) {
    let cwd_path = &compiler.context.config.root;
    extra_files.into_iter().for_each(|(path, content)| {
        fs::write(cwd_path.join(path), content).unwrap();
    });
}
