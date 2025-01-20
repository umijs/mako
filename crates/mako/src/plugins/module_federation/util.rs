use anyhow::{anyhow, Result};
use serde::{Serialize, Serializer};

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

pub(super) fn serialize_none_to_false<T: Serialize, S: Serializer>(
    t: &Option<T>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match t {
        Some(t) => t.serialize(s),
        None => s.serialize_bool(false),
    }
}
