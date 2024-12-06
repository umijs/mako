use anyhow::{anyhow, Result};

pub(super) fn parse_remote(remote: &str) -> Result<(String, String)> {
    let (left, right) = remote
        .split_once('@')
        .ok_or(anyhow!("invalid remote {}", remote))?;
    if left.is_empty() || right.is_empty() {
        Err(anyhow!("invalid remote {}", remote))
    } else {
        Ok((left.to_string(), right.to_string()))
    }
}
