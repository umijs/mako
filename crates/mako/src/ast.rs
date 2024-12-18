use swc_core::common::SyntaxContext;

pub(crate) mod comments;
pub(crate) mod css_ast;
pub(crate) mod error;
pub mod file;
pub(crate) mod js_ast;
pub(crate) mod sourcemap;
pub mod tests;
pub mod utils;

pub const DUMMY_CTXT: SyntaxContext = SyntaxContext::empty();
