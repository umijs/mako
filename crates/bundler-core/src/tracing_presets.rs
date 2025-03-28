use std::sync::LazyLock;
use turbopack_trace_utils::tracing_presets::{
    TRACING_OVERVIEW_TARGETS, TRACING_TURBOPACK_TARGETS, TRACING_TURBO_TASKS_TARGETS,
};

pub static TRACING_BUNDLER_OVERVIEW_TARGETS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    [
        &TRACING_OVERVIEW_TARGETS[..],
        &[
            "bundler_napi=info",
            "bundler=info",
            "bundler_api=info",
            "bundler_core=info",
            "turbopack_node=info",
        ],
    ]
    .concat()
});

pub static TRACING_BUNDLER_TARGETS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    [
        &TRACING_BUNDLER_OVERVIEW_TARGETS[..],
        &[
            "bundler_napi=trace",
            "bundler=trace",
            "bundler_api=trace",
            "bundler_core=trace",
        ],
    ]
    .concat()
});
pub static TRACING_BUNDLER_TURBOPACK_TARGETS: LazyLock<Vec<&str>> =
    LazyLock::new(|| [&TRACING_BUNDLER_TARGETS[..], &TRACING_TURBOPACK_TARGETS[..]].concat());
pub static TRACING_BUNDLER_TURBO_TASKS_TARGETS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    [
        &TRACING_BUNDLER_TURBOPACK_TARGETS[..],
        &TRACING_TURBO_TASKS_TARGETS[..],
    ]
    .concat()
});
