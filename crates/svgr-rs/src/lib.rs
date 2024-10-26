#![feature(path_file_prefix)]
#![deny(clippy::all)]

#[cfg(feature = "node")]
#[macro_use]
extern crate napi_derive;

use std::borrow::Borrow;
use std::sync::Arc;

use swc_core::common::comments::SingleThreadedComments;
use swc_core::common::{FileName, SourceMap};
use swc_core::ecma::codegen::text_writer::JsWriter;
use swc_core::ecma::codegen::Emitter;
use swc_core::ecma::visit::{as_folder, FoldWith};
use swc_xml::parser::parse_file_as_document;

mod add_jsx_attribute;
mod core;
mod error;
mod hast_to_swc_ast;
mod remove_jsx_attribute;
mod replace_jsx_attribute;
mod svg_dynamic_title;
mod svg_em_dimensions;
mod transform_react_native_svg;
mod transform_svg_component;

pub use error::SvgrError;

pub use self::core::config::{Config, ExpandProps, ExportType, Icon, JSXRuntime, JSXRuntimeImport};
pub use self::core::state::{Caller, Config as State};

/// Transform SVG into React components.
///
/// It takes three arguments:
///
/// * source: the SVG source code to transform
/// * options: the options used to transform the SVG
/// * state: a state linked to the transformation
///
/// # Examples
///
/// Basic usage:
///
/// ```rust
/// use svgr_rs::transform;
///
/// let result = transform(
///   r#"<svg></svg>"#.to_string(),
///   Default::default(),
///   Default::default(),
/// );
/// ```
pub fn transform(code: String, config: Config, state: State) -> Result<String, SvgrError> {
    let state = core::state::expand_state(&state);

    let cm = Arc::<SourceMap>::default();
    let fm = cm.new_source_file(FileName::Anon.into(), code);

    let mut errors = vec![];
    let document = parse_file_as_document(fm.borrow(), Default::default(), &mut errors)
        .map_err(|e| SvgrError::Parse(e.message().to_string()))?;

    let jsx_element = hast_to_swc_ast::to_swc_ast(document);
    if jsx_element.is_none() {
        return Err(SvgrError::InvalidSvg);
    }
    let jsx_element = jsx_element.unwrap();

    let m = transform_svg_component::transform(jsx_element, &config, &state)?;

    let m = m.fold_with(&mut as_folder(remove_jsx_attribute::Visitor::new(&config)));
    let m = m.fold_with(&mut as_folder(add_jsx_attribute::Visitor::new(&config)));

    let icon = match config.icon {
        Some(core::config::Icon::Bool(b)) => b,
        None => false,
        _ => true,
    };
    let dimensions = config.dimensions.unwrap_or(true);
    let m = if icon && dimensions {
        m.fold_with(&mut as_folder(svg_em_dimensions::Visitor::new(&config)))
    } else {
        m
    };

    let replace_attr_values = config.replace_attr_values.is_some();
    let m = if replace_attr_values {
        m.fold_with(&mut as_folder(replace_jsx_attribute::Visitor::new(&config)))
    } else {
        m
    };

    let title_prop = config.title_prop.unwrap_or(false);
    let m = if title_prop {
        m.fold_with(&mut as_folder(svg_dynamic_title::Visitor::new(
            "title".to_string(),
        )))
    } else {
        m
    };

    let desc_prop = config.desc_prop.unwrap_or(false);
    let m = if desc_prop {
        m.fold_with(&mut as_folder(svg_dynamic_title::Visitor::new(
            "desc".to_string(),
        )))
    } else {
        m
    };

    let native = config.native.unwrap_or(false);
    let m = if native {
        let comments = SingleThreadedComments::default();
        m.fold_with(&mut as_folder(transform_react_native_svg::Visitor::new(
            &comments,
        )))
    } else {
        m
    };

    let mut buf = vec![];

    let mut emitter = Emitter {
        cfg: Default::default(),
        cm: cm.clone(),
        comments: None,
        wr: JsWriter::new(cm, "\n", &mut buf, None),
    };
    emitter.emit_module(&m).unwrap();

    Ok(String::from_utf8_lossy(&buf).to_string())
}
