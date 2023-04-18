use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    context::Context,
    utils::file::{content_hash, ext_name, file_size, to_base64},
};
use std::collections::HashMap;

pub struct LoadParam<'a> {
    pub path: &'a str,
    pub files: Option<&'a HashMap<String, String>>,
}

pub enum ContentType {
    Js,
    Raw,
    File,
    NotMatch,
}

pub struct LoadResult {
    pub content: String,
    pub content_type: ContentType,
}

lazy_static! {
    static ref IMAGE_RE: Regex = Regex::new(r#"(jpg|jpeg|png|svg|gif)$"#).unwrap();
}

pub fn load(load_param: &LoadParam, _context: &mut Context) -> LoadResult {
    println!("> load {}", load_param.path);
    let ext_name = ext_name(load_param.path);
    match ext_name {
        "js" | "jsx" | "ts" | "tsx" => load_js(load_param, _context),
        "css" => load_css(load_param, _context),
        _ => {
            if IMAGE_RE.is_match(ext_name) {
                load_image(load_param, _context)
            } else {
                LoadResult {
                    content: "not match".to_string(),
                    content_type: ContentType::NotMatch,
                }
            }
        }
    }
}

fn load_js(load_param: &LoadParam, _context: &Context) -> LoadResult {
    if let Some(files) = load_param.files {
        if let Some(content) = files.get(load_param.path) {
            return LoadResult {
                content: content.clone(),
                content_type: ContentType::Js,
            };
        }
        panic!("File not found: {}", load_param.path);
    } else {
        LoadResult {
            content: std::fs::read_to_string(load_param.path).unwrap(),
            content_type: ContentType::Js,
        }
    }
}

fn load_css(load_param: &LoadParam, _context: &Context) -> LoadResult {
    let css_content = std::fs::read_to_string(load_param.path).unwrap();
    LoadResult {
        content: format!(
            r#"
const cssCode = `{}`;
const style = document.createElement('style');
style.innerHTML = cssCode;
document.head.appendChild(style);
"#,
            css_content
        ),
        content_type: ContentType::Js,
    }
}

fn load_image(load_param: &LoadParam, _context: &mut Context) -> LoadResult {
    // emit file like file-loader
    if file_size(load_param.path).unwrap() > _context.config.data_url_limit.try_into().unwrap() {
        let final_file_name =
            content_hash(load_param.path).unwrap() + "." + ext_name(load_param.path);
        // let final_file_path = _context.config.output.path.clone() + "/" + &final_file_name;
        // emit asset file
        _context.emit_assets(load_param.path.to_string(), final_file_name.clone());
        return LoadResult {
            content: format!("export default \"{}\"", final_file_name),
            content_type: ContentType::File,
        };
    }

    // handle file as Data URL, only support base64 now
    let base64_string = to_base64(load_param.path);
    LoadResult {
        content: format!("export default \"{}\"", base64_string.unwrap()),
        content_type: ContentType::Raw,
    }
}
