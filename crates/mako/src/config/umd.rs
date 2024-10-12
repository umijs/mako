use serde::Deserialize;

use crate::create_deserialize_fn;

pub type Umd = String;

create_deserialize_fn!(deserialize_umd, Umd);
