mod utils;
//
use wasm_bindgen::prelude::*;

use std::sync::{Arc, Once};
use mako_core::tokio;

use mako::compiler::{Args, Compiler};
use mako::config::Config;
use mako::plugin::Plugin;

#[wasm_bindgen]
extern {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn greet(root: &str) {
    utils::set_panic_hook();
    let root = std::path::PathBuf::from(root);
    let config = match Config::new(&root, None, None) {
        Ok(config) => config,
        Err(e) => {
            let msg = format!("config error {:?}", e);
            log(&msg);
            panic!("config failed");
        }
    };
    let mut plugins: Vec<Arc<dyn Plugin>> = vec![];
    let compiler = match Compiler::new(config, root.clone(), Args { watch: false }, Some(plugins)) {
        Ok(compiler) => compiler,
        Err(e) => {
            let msg = format!("compiler {:?}", e);
            log(&msg);
            panic!("compiler failed");
        }
    };

    if let Err(e) = compiler.compile() {
        let msg = format!("compiler {:?}", e);
        log(&msg);
    }
}