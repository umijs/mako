use std::process::Command;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

fn mako_with_antd(c: &mut Criterion) {
    c.bench_function("mako_with_antd", |b| {
        b.iter(|| {
            Command::new("target/release/mako")
                .current_dir("../..")
                .arg("examples/with-antd")
                .output()
                .expect("Failed to execute binary");
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20).measurement_time(Duration::from_secs(25)).warm_up_time(Duration::from_secs(1));
    targets = mako_with_antd
}
criterion_main!(benches);
