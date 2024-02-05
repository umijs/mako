pub use rustchash::{FxHashMap as HashMap, FxHashSet as HashSet};

mod rustchash {
    use std::collections::{HashMap, HashSet};
    use std::hash::BuildHasherDefault;

    use rustc_hash::FxHasher;

    pub type FxRandomState = BuildHasherDefault<FxHasher>;

    pub type FxHashMap<K, V> = HashMap<K, V, FxRandomState>;

    pub type FxHashSet<V> = HashSet<V, FxRandomState>;
}

#[macro_export(local_inner_macros)]
macro_rules! hashmap {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(hashmap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { hashmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let _cap = hashmap!(@count $($key),*);
            let mut _map = $crate::collections::HashMap::default();
            $(
                let _ = _map.insert($key, $value);
            )*
            _map
        }
    };
}
