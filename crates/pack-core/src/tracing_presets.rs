use std::sync::LazyLock;
use turbopack_trace_utils::tracing_presets::{
    TRACING_OVERVIEW_TARGETS as _TRACING_OVERVIEW_TARGETS,
    TRACING_TURBO_TASKS_TARGETS as _TRACING_TURBO_TASKS_TARGETS,
    TRACING_TURBOPACK_TARGETS as _TRACING_TURBOPACK_TARGETS,
};

pub static TRACING_OVERVIEW_TARGETS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    [
        &_TRACING_OVERVIEW_TARGETS[..],
        &[
            "pack_napi=info",
            "pack=info",
            "pack_api=info",
            "pack_core=info",
            "turbopack_node=info",
        ],
    ]
    .concat()
});

pub static TRACING_TARGETS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    [
        &TRACING_OVERVIEW_TARGETS[..],
        &[
            "pack_napi=trace",
            "pack=trace",
            "pack_api=trace",
            "pack_core=trace",
        ],
    ]
    .concat()
});
pub static TRACING_TURBOPACK_TARGETS: LazyLock<Vec<&str>> =
    LazyLock::new(|| [&TRACING_TARGETS[..], &_TRACING_TURBOPACK_TARGETS[..]].concat());
pub static TRACING_TURBO_TASKS_TARGETS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    [
        &TRACING_TURBOPACK_TARGETS[..],
        &_TRACING_TURBO_TASKS_TARGETS[..],
    ]
    .concat()
});
