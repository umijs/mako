use std::collections::HashMap;

use mako_bundler::{
    build::build::BuildParam, compiler::Compiler, config::Config, generate::generate::GenerateParam,
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
    insta::assert_debug_snapshot!(test_files(files));
}

fn test_files(files: HashMap<String, String>) -> Vec<String> {
    let mut config = Config::from_str(
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
    generate_result.output_files[0].__output.clone()
}
