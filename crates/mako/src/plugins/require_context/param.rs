use std::fmt::{Display, Formatter};
use std::path::Path;

use path_clean::PathClean;
use pathdiff::diff_paths;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use swc_core::ecma::ast::{Expr, ExprOrSpread, Lit, Regex};
use thiserror::Error;

use super::VIRTUAL_REQUIRE_CONTEXT_MODULE;
use crate::compiler::Context;

#[derive(Debug)]
pub struct ContextParam {
    pub rel_path: String,
    pub use_subdirectories: bool,
    pub reg_expr: Regex,
    pub mode: ContextLoadMode,
}

pub fn encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

impl ContextParam {
    pub fn to_context_id(&self, from: &Path, context: &Context) -> anyhow::Result<String> {
        let parent = from.parent().unwrap();
        let context_root = parent.join(&self.rel_path).clean();

        let relative_path = diff_paths(context_root, &context.root).unwrap_or_else(|| from.into());

        let ignore_case = self.reg_expr.flags.contains('i');

        Ok(format!(
            "{}?root={}&sub={}&reg={}&mode={}&ig={}",
            VIRTUAL_REQUIRE_CONTEXT_MODULE,
            encode(relative_path.to_string_lossy().as_ref(),),
            self.use_subdirectories,
            encode(self.reg_expr.exp.as_ref()),
            self.mode,
            ignore_case,
        ))
    }

    pub fn invalid(from: &Path) -> String {
        let parent = from.to_string_lossy().to_string();

        format!("virtual:context?from={}&invalid=true", parent)
    }
}

#[derive(Error, Debug)]
pub enum ParseContextLoadModeError {
    #[error("only sync, lazy, eager, weak, lazy-once are supported, got {0}")]
    UnsupportedValue(String),
}

/*
ref https://webpack.js.org/guides/dependency-management/#requirecontext
require.context(
  directory,
  (useSubdirectories = true),
  (regExp = /^\.\/.*$/),
  (mode = 'sync')
);
*/
pub struct ContextParamBuilder {
    valid: bool,
    rel_path: Option<String>,
    use_subdirectories: bool,
    reg_expr: Regex,
    mode: ContextLoadMode,
}

impl Default for ContextParamBuilder {
    fn default() -> Self {
        Self {
            valid: true,
            rel_path: None,
            use_subdirectories: true,
            reg_expr: Regex {
                span: Default::default(),
                exp: r#"^\.\/.*$"#.into(),
                flags: "".into(),
            },
            mode: ContextLoadMode::Sync,
        }
    }
}

impl TryFrom<&String> for ContextLoadMode {
    type Error = ParseContextLoadModeError;

    fn try_from(value: &String) -> anyhow::Result<Self, Self::Error> {
        match value.as_str() {
            "sync" => Ok(Self::Sync),
            "lazy" => Ok(Self::Lazy),
            "eager" => Ok(Self::Eager),
            "weak" => Ok(Self::Weak),
            "lazy-once" => Ok(Self::LazyOnce),
            _ => Err(Self::Error::UnsupportedValue(value.clone())),
        }
    }
}

impl ContextParamBuilder {
    pub fn relative_path(mut self, arg: Option<&ExprOrSpread>) -> ContextParamBuilder {
        if !self.valid {
            return self;
        }

        if let Some(&ExprOrSpread {
            expr: box Expr::Lit(Lit::Str(ref str)),
            ..
        }) = arg
        {
            self.rel_path = Some(str.value.to_string());
        } else {
            self.valid = false;
        }

        self
    }

    pub fn sub_directories(mut self, arg: Option<&ExprOrSpread>) -> ContextParamBuilder {
        if !self.valid {
            return self;
        }

        match arg {
            Some(ExprOrSpread {
                expr: box Expr::Lit(Lit::Bool(sub)),
                ..
            }) => {
                self.use_subdirectories = sub.value;
            }
            None => {}
            _ => {
                self.valid = false;
            }
        }
        self
    }

    pub fn mode(mut self, arg: Option<&ExprOrSpread>) -> ContextParamBuilder {
        if !self.valid {
            return self;
        }

        match &arg {
            Some(&ExprOrSpread {
                expr: box Expr::Lit(Lit::Str(ref mode_str)),
                ..
            }) => {
                let mode = mode_str.value.to_string();
                match ContextLoadMode::try_from(&mode) {
                    Ok(mode) => {
                        self.mode = mode;
                    }
                    Err(_err) => {
                        self.valid = false;
                    }
                };
            }
            None => {}

            _ => self.valid = false,
        }

        self
    }

    pub fn reg_expr(mut self, arg: Option<&ExprOrSpread>) -> ContextParamBuilder {
        if !self.valid {
            return self;
        }
        match arg {
            Some(&ExprOrSpread {
                expr: box Expr::Lit(Lit::Regex(ref reg)),
                ..
            }) => {
                self.reg_expr = reg.clone();
            }
            None => {}

            _ => {
                self.valid = false;
            }
        }

        self
    }

    pub fn build(self) -> Option<ContextParam> {
        if self.valid && self.rel_path.is_some() {
            Some(ContextParam {
                rel_path: self.rel_path.unwrap(),
                use_subdirectories: self.use_subdirectories,
                reg_expr: self.reg_expr,
                mode: self.mode,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ContextLoadMode {
    Sync,
    Lazy,
    Eager,
    Weak,
    LazyOnce,
}
impl Display for ContextLoadMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Sync => "sync",
                Self::Lazy => "lazy",
                Self::Eager => "eager",
                Self::Weak => "weak",
                Self::LazyOnce => "lazy-once",
            }
        )
    }
}
