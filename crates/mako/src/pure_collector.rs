use std::collections::HashSet;

use swc_common::comments::{Comment, CommentKind, Comments};
use swc_common::Span;
use swc_ecma_ast::Expr;

pub struct PureCollector<'a> {
    pure_top_level_ident: HashSet<String>,
    comments: Option<&'a dyn Comments>,
}

impl PureCollector<'_> {
    pub fn parse_expr(&self, _expr: Option<Box<Expr>>) {}
    /**
     * Check for `/*#__PURE__*/`
     */
    fn has_pure(&self, span: Span) -> bool {
        self.has_flag(span, "PURE")
    }

    /**
     * Check for `/*#__NO_SIDE_EFFECTS__*/`
     */
    fn has_no_side_effects(&self, span: Span) -> bool {
        self.has_flag(span, "NO_SIDE_EFFECTS")
    }

    fn find_comment<F>(&self, span: Span, mut op: F) -> bool
    where
        F: FnMut(&Comment) -> bool,
    {
        let mut found = false;
        if let Some(comments) = self.comments {
            let cs = comments.get_leading(span.lo);
            if let Some(cs) = cs {
                for c in &cs {
                    found |= op(c);
                    if found {
                        break;
                    }
                }
            }
        }

        found
    }

    fn has_flag(&self, span: Span, text: &'static str) -> bool {
        self.find_comment(span, |c| {
            if c.kind == CommentKind::Block {
                //
                if c.text.len() == (text.len() + 5)
                    && (c.text.starts_with("#__") || c.text.starts_with("@__"))
                    && c.text.ends_with("__")
                    && text == &c.text[3..c.text.len() - 2]
                {
                    return true;
                }
            }

            false
        })
    }
}
