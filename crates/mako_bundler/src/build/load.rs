use crate::context::Context;

pub struct LoadParam<'a> {
    pub path: &'a str,
}

pub struct LoadResult {
    pub content: String,
}

pub fn load(load_param: &LoadParam, _context: &Context) -> LoadResult {
    println!("> load {}", load_param.path);
    LoadResult {
        content: std::fs::read_to_string(load_param.path).unwrap(),
    }
}
