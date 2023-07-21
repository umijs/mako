use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub struct AssetsInfo {
    pub assets_type: String,
    pub size: u64,
    pub name: String,
    pub chunk_id: String,
    pub path: PathBuf,
}
#[derive(Debug)]
pub struct StatsInfo {
    // 产物信息
    pub assets: Vec<AssetsInfo>,
    // 代码文件大小
    pub file_content: HashMap<String, u64>,
    // 代码文件和 chunk_id 的关系
    pub file_with_chunk: HashMap<String, String>,
}

impl StatsInfo {
    pub fn new() -> Self {
        Self {
            assets: vec![],
            file_content: HashMap::new(),
            file_with_chunk: HashMap::new(),
        }
    }

    pub fn add_assets(&mut self, size: u64, name: String, chunk_id: String, path: PathBuf) {
        self.assets.push(AssetsInfo {
            assets_type: "assets".to_string(),
            size,
            name,
            chunk_id,
            path,
        });
    }

    pub fn add_file_content(&mut self, path: String, size: u64) {
        self.file_content.entry(path).or_insert(size);
    }
}

impl Default for StatsInfo {
    fn default() -> Self {
        Self::new()
    }
}
