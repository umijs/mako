use std::{collections::HashMap, vec};

use mako_bundler::{
    build::build::BuildParam, compiler::Compiler, config::Config,
    generate::generate::GenerateParam, module::ModuleId,
};

#[test]
fn normal() {
    let files = HashMap::from([
        (
            "/tmp/entry.js".to_string(),
            r###"
import {fn} from './foo';
console.log(fn());
            "###
            .to_string(),
        ),
        (
            "/tmp/foo.js".to_string(),
            r###"
export function fn() {
    return 123
}
            "###
            .to_string(),
        ),
    ]);
    let (output, _) = test_files(files);
    insta::assert_debug_snapshot!(output);
}

#[test]
fn multiple_files() {
    let files = HashMap::from([
        (
            "/tmp/entry.js".to_string(),
            r###"
import {three} from './three';
import {one} from './one';
import {two} from './two';
console.log(one());
            "###
            .to_string(),
        ),
        (
            "/tmp/one.js".to_string(),
            r###"
import {two} from './two';
export function one() {
    return two();
}
            "###
            .to_string(),
        ),
        (
            "/tmp/two.js".to_string(),
            r###"
export function two() {
    return 123
}
            "###
            .to_string(),
        ),
        (
            "/tmp/three.js".to_string(),
            r###"
export function three() {
    return 123
}
            "###
            .to_string(),
        ),
    ]);
    let (output, mut compiler) = test_files(files);
    insta::assert_debug_snapshot!(output);
    let orders = compiler.context.module_graph.topo_sort().unwrap();
    assert_eq!(
        &orders,
        &vec![
            ModuleId::new("/tmp/entry.js"),
            ModuleId::new("/tmp/three.js"),
            ModuleId::new("/tmp/one.js"),
            ModuleId::new("/tmp/two.js"),
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
    let files = HashMap::from([
        (
            "/tmp/entry.js".to_string(),
            r###"
import {one} from './one';
if (process.env.NODE_ENV === 'production') {
	console.log(123);
}
            "###
            .to_string(),
        ),
        (
            "/tmp/one.js".to_string(),
            r###"
function foo() {
	if (process.env.NODE_ENV === 'production') {
		console.log(123);
	}
	if (process.env['NODE_ENV'] === 'production') {
		console.log(123);
	}
	const test = process.env['NODE_ENV'];
}
            "###
            .to_string(),
        ),
    ]);
    let (output, _) = test_files(files);
    insta::assert_debug_snapshot!(output);
}

#[allow(clippy::useless_format)]
fn test_files(files: HashMap<String, String>) -> (Vec<String>, Compiler) {
    let mut config = Config::from(
        format!(
            r#"
{{
    "entry": {{
        "entry": "/tmp/entry.js"
    }},
    "root": "/tmp"
}}
            "#
        )
        .as_str(),
    )
    .unwrap();
    config.normalize();
    let mut compiler = Compiler::new(config);
    compiler.build(&BuildParam { files: Some(files) });
    let generate_result = compiler.generate(&GenerateParam { write: false });
    let output = generate_result.output_files[0].__output.clone();
    (output, compiler)
}
