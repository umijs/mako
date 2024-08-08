use std::cmp::Ordering;

pub fn compare_ids(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

pub fn compare_numbers(a: usize, b: usize) -> Ordering {
    a.cmp(&b)
}
