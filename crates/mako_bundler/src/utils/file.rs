use base64::{alphabet::STANDARD, engine, Engine};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    str,
};

pub fn ext_name(path: &str) -> &str {
    Path::new(path).extension().unwrap().to_str().unwrap()
}

pub fn file_size(file_path: &str) -> std::io::Result<u64> {
    let metadata = fs::metadata(file_path)?;
    Ok(metadata.len())
}

pub fn content_hash(file_path: &str) -> std::io::Result<String> {
    let file = fs::File::open(file_path).unwrap();
    // Find the length of the file
    let len = file.metadata().unwrap().len();
    // Decide on a reasonable buffer size (1MB in this case, fastest will depend on hardware)
    let buf_len = len.min(1_000_000) as usize;
    let mut buf = BufReader::with_capacity(buf_len, file);
    // webpack use md4
    let mut context = md5::Context::new();
    loop {
        // Get a chunk of the file
        let part = buf.fill_buf().unwrap();
        if part.is_empty() {
            break;
        }
        context.consume(part);
        // Tell the buffer that the chunk is consumed
        let part_len = part.len();
        buf.consume(part_len);
    }
    let digest = context.compute();
    Ok(format!("{:x}", digest))
}

pub fn to_base64(path: &str) -> std::io::Result<String> {
    let vec = fs::read(path)?;
    let engine = engine::GeneralPurpose::new(&STANDARD, engine::general_purpose::PAD);

    let base64 = engine.encode(&vec);

    // 直接用 extension 可能处理不了 jpeg 格式的情况
    let file_type = ext_name(path);
    Ok(format!(
        "data:image/{};base64,{}",
        file_type,
        base64.replace("\r\n", "")
    ))
}
