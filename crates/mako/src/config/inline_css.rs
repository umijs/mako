use serde::{Deserialize, Serialize};

use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Debug)]
pub struct InlineCssConfig {}

create_deserialize_fn!(deserialize_inline_css, InlineCssConfig);
