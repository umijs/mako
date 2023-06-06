use std::{
    fs,
    io::{BufRead, BufReader},
};

pub fn content_hash(file_path: &str) -> anyhow::Result<String> {
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

pub fn content_hash_with_len(file_path: &str, len: usize) -> anyhow::Result<String> {
    let hash = content_hash(file_path)?;
    Ok(hash[..len].to_string())
}
