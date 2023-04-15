use std::{fs::read, path::Path};

use regex::Regex;
use lazy_static::lazy_static;
use rustc_serialize::base64::{ToBase64, MIME};

use crate::context::Context;

pub struct LoadParam<'a> {
    pub path: &'a str,
}

pub struct LoadResult {
    pub content: String,
}

pub fn load(load_param: &LoadParam, _context: &Context) -> LoadResult {
    lazy_static! {
        static ref IMAGE_RE: Regex = Regex::new(r#"(jpg|jpeg|png|svg|gif)$"#).unwrap();
    }

    println!("> load {}", load_param.path);
	let ext_name = get_ext_name(load_param.path);
	if IMAGE_RE.is_match(ext_name) {
		load_image(load_param, _context)
	} else {
		load_js(load_param, _context)
	}
}

fn get_ext_name(path: &str) -> &str {
	Path::new(path).extension().unwrap().to_str().unwrap()
}

fn load_js(load_param: &LoadParam, _context: &Context) -> LoadResult {
    LoadResult {
        content: std::fs::read_to_string(load_param.path).unwrap(),
    }
}

fn load_image(load_param: &LoadParam, _context: &Context) -> LoadResult {
	let base64_string = to_base64(load_param.path);
	let image_module_vec = vec![
		"export default ",
		"\"",
		base64_string.as_str(),
		"\""
	];
	println!("image_js_module_content: {}", image_module_vec.join(""));
	LoadResult {
		content: image_module_vec.join(""),
	}
}

fn to_base64(path: &str) -> String {
    let vec = read(path).unwrap();
    let base64 = vec.to_base64(MIME);
    let file_type = Path::new(path).extension().unwrap()
        .to_str().unwrap();
    format!("data:image/{};base64,{}", file_type, base64.replace("\r\n", ""))
}
