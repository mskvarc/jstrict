use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use json_syntax::{Parse, Print, Value};

mod common;

fn fixtures() -> Vec<(&'static str, String)> {
	vec![
		("medium", common::medium()),
		("large", common::large()),
		("number_heavy_1k", common::number_heavy(1_000)),
	]
}

fn bench_canonicalize(c: &mut Criterion) {
	for (name, source) in fixtures() {
		let value = Value::parse_str(&source).unwrap().0;

		let mut group = c.benchmark_group(format!("canonicalize/{}", name));
		group.throughput(Throughput::Bytes(source.len() as u64));

		group.bench_with_input(BenchmarkId::new("default", ""), &value, |b, v| {
			b.iter_batched(
				|| v.clone(),
				|mut clone| {
					black_box(&mut clone).canonicalize();
					clone
				},
				criterion::BatchSize::SmallInput,
			);
		});

		group.bench_with_input(BenchmarkId::new("buffer_reuse", ""), &value, |b, v| {
			let mut buffer = ryu_js::Buffer::new();
			b.iter_batched(
				|| v.clone(),
				|mut clone| {
					black_box(&mut clone).canonicalize_with(&mut buffer);
					clone
				},
				criterion::BatchSize::SmallInput,
			);
		});

		group.finish();
	}
}

fn bench_pipeline(c: &mut Criterion) {
	let source = common::medium();

	c.bench_function("canonicalize/pipeline_medium", |b| {
		b.iter(|| {
			let mut v = Value::parse_str(black_box(source.as_str())).unwrap().0;
			v.canonicalize();
			v.compact_print().to_string()
		});
	});
}

criterion_group!(benches, bench_canonicalize, bench_pipeline);
criterion_main!(benches);
