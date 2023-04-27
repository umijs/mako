use std::vec;

use mako_bundler::{
    build::build::BuildParam, compiler::Compiler, config::Config,
    generate::generate::GenerateParam, module::ModuleId,
};

#[test]
fn normal() {
    let (output, ..) = test_files("normal".into());
    assert_debug_snapshot!(output);
}

#[test]
fn multiple_files() {
    let (output, mut compiler, cwd) = test_files("multiple".into());
    assert_debug_snapshot!(output);
    let (orders, _) = compiler.context.module_graph.topo_sort();
    assert_eq!(
        &orders,
        &vec![
            ModuleId::new(format!("{}/entry.js", cwd).as_str()),
            ModuleId::new(format!("{}/three.js", cwd).as_str()),
            ModuleId::new(format!("{}/one.js", cwd).as_str()),
            ModuleId::new(format!("{}/two.js", cwd).as_str()),
        ]
    );
    let mut vecs = vec![];
    for module_id in orders {
        let deps = compiler.context.module_graph.get_dependencies(&module_id);
        vecs.push((module_id, deps));
    }
    insta::assert_debug_snapshot!(&vecs);
}

#[test]
fn replace_env() {
    let (output, ..) = test_files("env".into());
    assert_debug_snapshot!(output);
}

#[test]
fn chunk() {
    let (output, compiler, ..) = test_files("chunks".into());
    assert_debug_snapshot!(output);
    assert_display_snapshot!(compiler.context.chunk_graph);
}

#[allow(clippy::useless_format)]
fn test_files(name: String) -> (Vec<String>, Compiler, String) {
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
    "root": "{}"
}}
            "#,
            cwd,
        )
        .as_str(),
    )
    .unwrap();
    config.normalize();
    let mut compiler = Compiler::new(config);
    compiler.build(&BuildParam { files: None });
    let generate_result = compiler.generate(&GenerateParam { write: false });
    let output = generate_result.output_files[0].__output.clone();
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
