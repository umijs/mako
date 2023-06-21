use std::process::Command;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

fn multiple_entries_heavy(c: &mut Criterion) {
    c.bench_function("multiple_entries_heavy", |b| {
        b.iter(|| {
            Command::new("target/release/mako")
                .current_dir("../..")
                .arg("examples/multiple-entries-heavy")
                .output()
                .expect("Failed to execute binary");
        });
    });
}

fn multiple_entries_heavy_min(c: &mut Criterion) {
    c.bench_function("multiple_entries_heavy_min", |b| {
        b.iter(|| {
            Command::new("target/release/mako")
                .current_dir("../..")
                .arg("--mode")
                .arg("production")
                .arg("examples/multiple-entries-heavy")
                .output()
                .expect("Failed to execute binary");
        });
    });
}

fn with_antd(c: &mut Criterion) {
    c.bench_function("with_antd", |b| {
        b.iter(|| {
            Command::new("target/release/mako")
                .current_dir("../..")
                .arg("examples/with-antd")
                .output()
                .expect("Failed to execute binary");
        });
    });
}

fn with_antd_min(c: &mut Criterion) {
    c.bench_function("with_antd_min", |b| {
        b.iter(|| {
            Command::new("target/release/mako")
                .current_dir("../..")
                .arg("--mode")
                .arg("production")
                .arg("examples/with-antd")
                .output()
                .expect("Failed to execute binary");
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(60)).warm_up_time(Duration::from_secs(1));
    targets = multiple_entries_heavy, multiple_entries_heavy_min, with_antd, with_antd_min
}
criterion_main!(benches);
