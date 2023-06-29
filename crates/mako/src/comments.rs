use swc_atoms::atom;
use swc_common::{BytePos, Span, DUMMY_SP};
use swc_node_comments::SwcComments;

// #[derive(Default)]
pub struct Comments(SwcComments);

impl Comments {
    pub fn new() -> Self {
        Self(SwcComments::default())
    }

    pub fn get_swc_comments(&self) -> &SwcComments {
        &self.0
    }

    pub fn add_unused_comment(&mut self, pos: BytePos) {
        let mut leading = self.0.leading.entry(pos).or_default();
        let unused_comment = swc_common::comments::Comment {
            kind: swc_common::comments::CommentKind::Block,
            span: DUMMY_SP,
            text: atom!("#__UNUSED__"),
        };

        if !leading.iter().any(|c| c.text == unused_comment.text) {
            leading.push(unused_comment);
        }
    }

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

    fn has_flag(&self, span: Span, text: &'static str) -> bool {
        self.find_comment(span, |c| {
            if c.kind == swc_common::comments::CommentKind::Block {
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

    fn find_comment<F>(&self, span: Span, mut op: F) -> bool
    where
        F: FnMut(&swc_common::comments::Comment) -> bool,
    {
        let mut found = false;
        let cs: Option<_> = swc_common::comments::Comments::get_leading(&self.0, span.lo);
        if let Some(cs) = cs {
            for c in &cs {
                found |= op(c);
                if found {
                    break;
                }
            }
        }

        found
    }
}
