use std::collections::HashMap;

// format: HashMap<identifier, (import_source, specifier)>
// e.g.
// { "process": ("process", "") }
// { "Buffer": ("buffer", "Buffer") }
pub type Providers = HashMap<String, (String, String)>;
