use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{BinExpr, BinaryOp, Expr, Lit};
use swc_core::ecma::utils::{member_expr, quote_str};
use swc_core::ecma::visit::VisitMut;

use crate::ast::file::File;
use crate::ast::{utils, DUMMY_CTXT};
use crate::build::load::Load;
use crate::compiler::Context;
use crate::config::Platform;
use crate::module::{Dependency, ResolveType};
use crate::resolve;

pub struct NewUrlAssets {
    pub context: Arc<Context>,
    pub path: PathBuf,
    pub unresolved_mark: Mark,
}

impl NewUrlAssets {
    fn handle_asset(&self, url: String) -> Result<String> {
        let dep = Dependency {
            source: url,
            resolve_as: None,
            resolve_type: ResolveType::Css,
            order: 0,
            span: None,
        };
        let resolved = resolve::resolve(
            self.path.to_string_lossy().as_ref(),
            &dep,
            &self.context.resolvers,
            &self.context,
        )?;
        let resolved_path = resolved.get_resolved_path();
        Load::handle_asset(
            &File::new(resolved_path.clone(), self.context.clone()),
            false,
            false,
            self.context.clone(),
        )
    }

    fn build_import_meta_url(&self, context: Arc<Context>) -> Expr {
        let is_browser = matches!(context.config.platform, Platform::Browser);
        if is_browser {
            Expr::Bin(BinExpr {
                span: DUMMY_SP,
                op: BinaryOp::LogicalOr,
                left: member_expr!(DUMMY_CTXT, DUMMY_SP, document.baseURI).into(),
                right: member_expr!(DUMMY_CTXT, DUMMY_SP, self.location.href).into(),
            })
        } else {
            Expr::Lit(
                quote_str!(format!(
                    "file://{}",
                    self.path.to_string_lossy().to_string()
                ))
                .into(),
            )
        }
    }
}

impl VisitMut for NewUrlAssets {
    fn visit_mut_new_expr(&mut self, n: &mut swc_core::ecma::ast::NewExpr) {
        // new URL('', import.meta.url)
        if let box Expr::Ident(ident) = &n.callee {
            #[allow(clippy::needless_borrow)]
            if utils::is_ident_undefined(&ident, "URL", &self.unresolved_mark) {
                let args = n.args.as_mut().unwrap();
                if args
                    .get(1)
                    .is_some_and(|arg| utils::is_import_meta_url(&arg.expr))
                {
                    if let box Expr::Lit(Lit::Str(ref url)) = &args[0].expr {
                        if !utils::is_remote_or_data(&url.value) {
                            let origin = url.value.to_string();
                            let url = self.handle_asset(origin.clone());
                            if url.is_err() {
                                eprintln!("Failed to handle asset: {}", origin);
                            }
                            let url = url.unwrap_or(origin);
                            let is_browser =
                                matches!(self.context.config.platform, Platform::Browser);
                            args[0].expr = if is_browser {
                                Expr::Bin(BinExpr {
                                    span: DUMMY_SP,
                                    op: BinaryOp::Add,
                                    left: member_expr!(
                                        DUMMY_CTXT,
                                        DUMMY_SP,
                                        __mako_require__.publicPath
                                    )
                                    .into(),
                                    right: Lit::Str(url.into()).into(),
                                })
                                .into()
                            } else {
                                Lit::Str(url.into()).into()
                            };
                            args[1].expr = self.build_import_meta_url(self.context.clone()).into();
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::visit::VisitMutWith;

    use super::NewUrlAssets;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_normal() {
        assert_eq!(
            run(r#"new URL('big.jpg', import.meta.url)"#),
            r#"new URL(__mako_require__.publicPath + "big.8e6c05c3.jpg", document.baseURI || self.location.href);"#
        )
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let current_dir = std::env::current_dir().unwrap();
            let path = current_dir.join("src/visitors/fixtures/css_assets/test.js");
            let mut visitor = NewUrlAssets {
                context: test_utils.context.clone(),
                unresolved_mark: ast.unresolved_mark,
                path,
            };
            ast.ast.visit_mut_with(&mut visitor);
        });
        test_utils.js_ast_to_code()
    }
}
