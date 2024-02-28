use std::fmt;
use std::sync::{Arc, Mutex};

use mako_core::swc_common::Span;
use mako_core::swc_error_reporters::{GraphicalReportHandler, PrettyEmitter, PrettyEmitterConfig};
use mako_core::thiserror::Error;
use swc_core::common::errors::Handler;

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

pub fn code_frame(span: Span, message: &str, context: Arc<Context>) -> String {
    let cm = context.meta.css.cm.clone();
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
