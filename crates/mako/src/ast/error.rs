use std::fmt;
use std::sync::{Arc, Mutex};

use swc_core::common::errors::Handler;
use swc_core::common::Span;
use swc_error_reporters::{GraphicalReportHandler, PrettyEmitter, PrettyEmitterConfig};
use thiserror::Error;

use crate::compiler::Context;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("{messages:}")]
    JsParseError { messages: String },
    #[error("{messages:}")]
    CSSParseError { messages: String },
}

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("{message:}")]
    JsGenerateError { message: String },
    #[error("{message:}")]
    CSSGenerateError { message: String },
}

pub enum ErrorSpan {
    Js(Span),
    Css(Span),
}

pub fn code_frame(span: ErrorSpan, message: &str, context: Arc<Context>) -> String {
    let (span, cm) = match span {
        ErrorSpan::Js(span) => (span, context.meta.script.cm.clone()),
        ErrorSpan::Css(span) => (span, context.meta.css.cm.clone()),
    };
    let wr = Box::<LockedWriter>::default();
    let emitter = PrettyEmitter::new(
        cm,
        wr.clone(),
        GraphicalReportHandler::new().with_context_lines(3),
        PrettyEmitterConfig {
            skip_filename: false,
        },
    );
    let handler = Handler::with_emitter(true, false, Box::new(emitter));
    let mut db = handler.struct_span_err(span, message);
    // span.note(format!("Parse file failed: {}", path).as_str());
    db.emit();
    let s = &**wr.0.lock().unwrap();
    s.to_string()
}

#[derive(Clone, Default)]
struct LockedWriter(Arc<Mutex<String>>);

impl fmt::Write for LockedWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.lock().unwrap().push_str(s);
        Ok(())
    }
}
