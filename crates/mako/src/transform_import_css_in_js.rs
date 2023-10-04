use swc_ecma_ast::{ImportDecl, Str};
use swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct ImportCssInJs;

fn is_css_modules_path(path: &str) -> bool {
    path.ends_with(".module.css") || path.ends_with(".module.less")
}

fn is_css_path(path: &str) -> bool {
    path.ends_with(".css") || path.ends_with(".less")
}

impl VisitMut for ImportCssInJs {
    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        // TODO: add config
        // specifiers为空是普通的import, 否则为importfrom
        if is_css_modules_path(&import_decl.src.value)
            || (is_css_path(&import_decl.src.value) && !&import_decl.specifiers.is_empty())
        {
            self.replace_source(&mut import_decl.src);
        }
        import_decl.visit_mut_children_with(self);
    }
}

impl ImportCssInJs {
    fn replace_source(&mut self, source: &mut Str) {
        let to_replace = format!("{}?asmodule", &source.value.to_string());
        let span = source.span;
        *source = Str::from(to_replace);
        source.span = span;
    }
}
