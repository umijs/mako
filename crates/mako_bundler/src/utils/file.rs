use std::collections::hash_map::DefaultHasher;
use std::{fs, path::Path};
use std::hash::{Hash, Hasher};
use rustc_serialize::base64::{ToBase64, MIME};

pub fn copy_file(source_path: &str, target_path: &str) -> std::io::Result<()> {
    fs::copy(source_path, target_path)?;
    Ok(())
}

pub fn ext_name(path: &str) -> &str {
	Path::new(path).extension().unwrap().to_str().unwrap()
}

pub fn file_size(file_path: &str) -> std::io::Result<u64> {
	let metadata = fs::metadata(file_path)?;
	Ok(metadata.len())
}

pub fn content_hash(file_path: &str) -> std::io::Result<u64> {
	let file_string = fs::read_to_string(file_path)?;
    let mut hasher = DefaultHasher::new();
	file_string.hash(&mut hasher);
    Ok(hasher.finish())
}

pub fn to_base64(path: &str) -> std::io::Result<String> {
    let vec = fs::read(path)?;
    let base64 = vec.to_base64(MIME);
	// 直接用 extension 可能处理不了 jpeg 格式的情况
    let file_type = Path::new(path).extension().unwrap()
        .to_str().unwrap();
    Ok(format!("data:image/{};base64,{}", file_type, base64.replace("\r\n", "")))
}
