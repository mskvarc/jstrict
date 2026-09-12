use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

mod common;

fn bench_traverse(c: &mut Criterion) {
	let (medium, _) = common::parsed(&common::medium());

	c.bench_function("access/traverse/medium", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for _ in black_box(&medium).traverse() {
				n += 1;
			}
			n
		});
	});

	c.bench_function("access/volume/medium", |b| {
		b.iter(|| black_box(&medium).volume());
	});

	c.bench_function("access/array_filter_strings/medium", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for (_, frag) in black_box(&medium).traverse() {
				if frag.is_string() {
					n += 1;
				}
			}
			n
		});
	});
}

fn bench_object_lookup(c: &mut Criterion) {
	let json = common::wide_object(1_000);
	let value = common::parsed(&json).0;
	let Some(object) = value.as_object() else {
		return;
	};

	let keys: Vec<String> = (0..1_000).map(|i| format!("key_{i}")).collect();

	c.bench_function("access/object_get/wide_1k", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for k in &keys {
				if object.get(black_box(k.as_str())).next().is_some() {
					n += 1;
				}
			}
			n
		});
	});

	c.bench_function("access/object_contains_key/wide_1k", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for k in &keys {
				if object.contains_key(black_box(k.as_str())) {
					n += 1;
				}
			}
			n
		});
	});

	let missing: Vec<String> = (0..1_000).map(|i| format!("missing_{i}")).collect();
	c.bench_function("access/object_get_missing/wide_1k", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for k in &missing {
				if object.get(black_box(k.as_str())).next().is_some() {
					n += 1;
				}
			}
			n
		});
	});
}

fn bench_get_fragment(c: &mut Criterion) {
	let (medium, _) = common::parsed(&common::medium());
	let total = medium.volume();
	let mut rng = common::Rng::new(common::SEED);
	let indices: Vec<usize> = (0..1_000)
		.map(|_| usize::try_from(rng.range(total as u64)).unwrap_or(0))
		.collect();

	c.bench_function("access/get_fragment/medium", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for idx in &indices {
				if medium.get_fragment(black_box(*idx)).is_ok() {
					n += 1;
				}
			}
			n
		});
	});
}

criterion_group!(
	benches,
	bench_traverse,
	bench_object_lookup,
	bench_get_fragment
);
criterion_main!(benches);
