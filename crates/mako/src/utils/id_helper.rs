use std::collections::HashSet;

use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;
use crate::utils::comparators::{compare_ids, compare_numbers};

pub fn get_used_module_ids_and_modules() -> (HashSet<String>, Vec<String>) {
    let modules = vec![];
    let used_ids = HashSet::new();

    (used_ids, modules)
}

// Port from https://github.com/web-infra-dev/rspack/blob/7e47dcee91c13e32ef3adbc4df479a09eae18c14/crates/rspack_util/src/number_hash.rs

const SAFE_LIMIT: usize = 0x80000000usize;
const SAFE_PART: usize = SAFE_LIMIT - 1;
const COUNT: usize = 4;

pub fn get_number_hash(str: &str, range: usize) -> usize {
    let mut arr = [0usize; COUNT];
    let primes = [3usize, 7usize, 17usize, 19usize];

    for i in 0..str.len() {
        let c = str.as_bytes()[i] as usize;
        arr[0] = (arr[0] + c * primes[0] + arr[3]) & SAFE_PART;
        arr[1] = (arr[1] + c * primes[1] + arr[0]) & SAFE_PART;
        arr[2] = (arr[2] + c * primes[2] + arr[1]) & SAFE_PART;
        arr[3] = (arr[3] + c * primes[3] + arr[2]) & SAFE_PART;

        arr[0] ^= arr[arr[0] % COUNT] >> 1;
        arr[1] ^= arr[arr[1] % COUNT] >> 1;
        arr[2] ^= arr[arr[2] % COUNT] >> 1;
        arr[3] ^= arr[arr[3] % COUNT] >> 1;
    }

    if range <= SAFE_PART {
        (arr[0] + arr[1] + arr[2] + arr[3]) % range
    } else {
        let range_ext = range / SAFE_LIMIT;
        let sum1 = (arr[0] + arr[2]) & SAFE_PART;
        let sum2 = (arr[0] + arr[2]) % range_ext;
        (sum2 * SAFE_LIMIT + sum1) % range
    }
}

pub fn compare_modules_by_pre_order_index_or_identifier(
    module_graph: &ModuleGraph,
    a: &ModuleId,
    b: &ModuleId,
) -> std::cmp::Ordering {
    if let Some(a) = module_graph.id_index_map.get(a)
        && let Some(b) = module_graph.id_index_map.get(b)
    {
        compare_numbers(a.index(), b.index())
    } else {
        compare_ids(a.id.as_str(), b.id.as_str())
    }
}
