use mako_bundler::{
    build::build::BuildParam, compiler::Compiler, config::Config,
    generate::generate::GenerateParam, plugin::plugin_driver::PluginDriver,
};
use tracing::debug;

#[tokio::test(flavor = "multi_thread")]
async fn normal() {
    let (output, ..) = test_files("normal".into());
    assert_debug_snapshot!(output);
}

#[tokio::test(flavor = "multi_thread")]
async fn external() {
    let (output, ..) = test_files("external".into());
    assert_debug_snapshot!(output);
}

#[tokio::test(flavor = "multi_thread")]
async fn multiple_files() {
    let (output, compiler, ..) = test_files("multiple".into());
    assert_debug_snapshot!(output);
    let mut module_graph = compiler.context.module_graph.write().unwrap();
    let (orders, _) = module_graph.topo_sort();
    assert_debug_snapshot!(&orders);
    assert_display_snapshot!(&module_graph);
}

#[tokio::test(flavor = "multi_thread")]
async fn replace_env() {
    let (output, ..) = test_files("env".into());
    assert_debug_snapshot!(output);
}

#[tokio::test(flavor = "multi_thread")]
async fn chunk() {
    let (output, compiler, ..) = test_files("chunks".into());
    assert_debug_snapshot!(output);
    let chunk_graph = compiler.context.chunk_graph.read().unwrap();
    debug!("{}", &chunk_graph);
    assert_display_snapshot!(chunk_graph);
}

#[allow(clippy::useless_format)]
fn test_files(name: String) -> (Vec<Vec<String>>, Compiler, String) {
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
    let mut compiler = Compiler::new(config, PluginDriver::new());
    compiler.build(&BuildParam { files: None });
    let generate_result = compiler.generate(&GenerateParam { write: false });
    let output = generate_result
        .output_files
        .into_iter()
        .map(|f| f.__output)
        .collect::<Vec<Vec<String>>>();
    (output, compiler, cwd)
}

#[macro_export]
macro_rules! assert_display_snapshot {
    ($value:expr) => {{
        let cwd = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let value = format!("{}", $value).replace(&cwd, "<CWD>");
        insta::assert_snapshot!(insta::_macro_support::AutoName, value, stringify!($value));
    }};
}

#[macro_export]
macro_rules! assert_debug_snapshot {
    ($value:expr) => {{
        let cwd = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let value = format!("{:#?}", $value).replace(&cwd, "<CWD>");
        insta::assert_snapshot!(insta::_macro_support::AutoName, value, stringify!($value));
    }};
}
