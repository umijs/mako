use delegate::delegate;
use mako_core::swc_common;
use mako_core::swc_common::comments::{Comment, Comments as CommentsTrait};
use mako_core::swc_common::{BytePos, Span};
use mako_core::swc_node_comments::SwcComments;
use mako_core::tracing::warn;

#[derive(Default)]
pub struct Comments(MakoComments);

impl Comments {
    pub fn get_swc_comments(&self) -> &MakoComments {
        &self.0
    }

    pub fn add_leading_comment_at(&mut self, pos: BytePos, comment: Comment) {
        self.0.add_leading(pos, comment);
    }

    /**
     * Check for `/*#__UNUSED__*/`
     */
    #[allow(dead_code)]
    pub fn has_unused(&self, span: Span) -> bool {
        self.has_flag(span, "UNUSED")
    }

    /**
     * Check for `/*#__UNUSED_MODULE__*/`
     */
    #[allow(dead_code)]
    pub fn has_unused_module(&self, span: Span) -> bool {
        self.has_flag(span, "UNUSED_MODULE")
    }

    /**
     * Check for `/*#__PURE__*/`
     */
    #[allow(dead_code)]
    pub fn has_pure(&self, span: Span) -> bool {
        self.has_flag(span, "PURE")
    }

    /**
     * Check for `/*#__NO_SIDE_EFFECTS__*/`
     */
    #[allow(dead_code)]
    fn has_no_side_effects(&self, span: Span) -> bool {
        self.has_flag(span, "NO_SIDE_EFFECTS")
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

#[derive(Clone, Default)]
pub struct MakoComments(SwcComments);

impl CommentsTrait for MakoComments {
    fn add_pure_comment(&self, pos: BytePos) {
        //ref: https://github.com/swc-project/swc/pull/8172
        if pos.is_dummy() {
            #[cfg(debug_assertions)]
            {
                warn!("still got pure comments at dummy pos! UPGRADE SWC!!!");
            }
            return;
        }
        self.0.add_pure_comment(pos);
    }

    delegate! {
        to self.0 {
            fn add_leading(&self, pos: BytePos, cmt: Comment);
            fn add_leading_comments(&self, pos: BytePos, comments: Vec<Comment>);
            fn has_leading(&self, pos: BytePos) -> bool;
            fn move_leading(&self, from: BytePos, to: BytePos);
            fn take_leading(&self, pos: BytePos) -> Option<Vec<Comment>>;
            fn get_leading(&self, pos: BytePos) -> Option<Vec<Comment>>;
            fn add_trailing(&self, pos: BytePos, cmt: Comment);
            fn add_trailing_comments(&self, pos: BytePos, comments: Vec<Comment>);
            fn has_trailing(&self, pos: BytePos) -> bool;
            fn move_trailing(&self, from: BytePos, to: BytePos);
            fn take_trailing(&self, pos: BytePos) -> Option<Vec<Comment>>;
            fn get_trailing(&self, pos: BytePos) -> Option<Vec<Comment>>;
        }
    }
}
