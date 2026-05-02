use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use jstrict::{Parse, Value, parse::Options};
use std::hint::black_box;

mod common;

/// Fixtures with serde_json baseline (within serde_json's default recursion limit).
fn fixtures_with_baseline() -> Vec<(&'static str, String)> {
	vec![
		("tiny", common::tiny()),
		("small", common::small()),
		("medium", common::medium()),
		("large", common::large()),
		("wide_object_1k", common::wide_object(1_000)),
		("wide_array_10k", common::wide_array(10_000)),
		("number_heavy_1k", common::number_heavy(1_000)),
		("string_heavy_1k", common::string_heavy(1_000, 0.25)),
		("duplicate_keys", common::duplicate_keys(64, 16)),
	]
}

/// Pathological-depth fixtures — serde_json's default recursion limit (128) rejects these.
fn deep_fixtures() -> Vec<(&'static str, String)> {
	vec![
		("deep_array_256", common::deep_array(256)),
		("deep_object_256", common::deep_object(256)),
	]
}

fn bench_parse_group(c: &mut Criterion, name: &str, input: &str, with_baseline: bool) {
	let mut group = c.benchmark_group(format!("parse/{}", name));
	group.throughput(Throughput::Bytes(input.len() as u64));

	group.bench_with_input(BenchmarkId::new("json_syntax", "strict"), input, |b, s| {
		b.iter(|| Value::parse_str(black_box(s)).unwrap());
	});

	let bytes = input.as_bytes().to_vec();
	group.bench_with_input(BenchmarkId::new("json_syntax", "slice"), &bytes, |b, s| {
		b.iter(|| Value::parse_slice(black_box(s.as_slice())).unwrap());
	});

	let opts = Options::flexible();
	group.bench_with_input(
		BenchmarkId::new("json_syntax", "flexible"),
		input,
		|b, s| {
			b.iter(|| Value::parse_str_with(black_box(s), opts).unwrap());
		},
	);

	if with_baseline {
		group.bench_with_input(BenchmarkId::new("serde_json", "from_str"), input, |b, s| {
			b.iter(|| serde_json::from_str::<serde_json::Value>(black_box(s)).unwrap());
		});
	}

	group.finish();
}

fn bench_parse(c: &mut Criterion) {
	for (name, input) in fixtures_with_baseline() {
		bench_parse_group(c, name, &input, true);
	}
	for (name, input) in deep_fixtures() {
		bench_parse_group(c, name, &input, false);
	}
}

fn bench_parse_pathological(c: &mut Criterion) {
	let mut group = c.benchmark_group("parse_pathological");

	let deep = common::deep_array(1024);
	group.throughput(Throughput::Bytes(deep.len() as u64));
	group.bench_function("deep_array_1024", |b| {
		b.iter(|| Value::parse_str(black_box(deep.as_str())).unwrap())
	});

	let huge = common::huge_number(1024);
	group.throughput(Throughput::Bytes(huge.len() as u64));
	group.bench_function("huge_number_1kb", |b| {
		b.iter(|| Value::parse_str(black_box(huge.as_str())).unwrap())
	});

	let escapes = {
		let mut s = String::from("\"");
		for _ in 0..2_000 {
			s.push_str("\\u00ff");
		}
		s.push('"');
		s
	};
	group.throughput(Throughput::Bytes(escapes.len() as u64));
	group.bench_function("unicode_escape_heavy", |b| {
		b.iter(|| Value::parse_str(black_box(escapes.as_str())).unwrap())
	});

	let surrogates = common::surrogate_heavy(1_000);
	group.throughput(Throughput::Bytes(surrogates.len() as u64));
	group.bench_function("surrogate_pair_heavy_strict", |b| {
		b.iter(|| Value::parse_str(black_box(surrogates.as_str())).unwrap())
	});
	let flex = Options::flexible();
	group.bench_function("surrogate_pair_heavy_flexible", |b| {
		b.iter(|| Value::parse_str_with(black_box(surrogates.as_str()), flex).unwrap())
	});

	group.finish();
}

/// Mirror of `bench_parse` but using `parse_str_value` / `parse_slice_value`,
/// the no-`CodeMap` entry points. This delta vs `parse/*` measures the
/// `Mapper` monomorphisation win.
fn bench_parse_value(c: &mut Criterion) {
	for (name, input) in fixtures_with_baseline() {
		let mut group = c.benchmark_group(format!("parse_value/{}", name));
		group.throughput(Throughput::Bytes(input.len() as u64));

		group.bench_with_input(
			BenchmarkId::new("json_syntax", "str_value"),
			input.as_str(),
			|b, s| b.iter(|| jstrict::parse::parse_str_value(black_box(s)).unwrap()),
		);

		let bytes = input.as_bytes().to_vec();
		group.bench_with_input(
			BenchmarkId::new("json_syntax", "slice_value"),
			&bytes,
			|b, s| b.iter(|| jstrict::parse::parse_slice_value(black_box(s.as_slice())).unwrap()),
		);

		group.bench_with_input(
			BenchmarkId::new("serde_json", "from_str"),
			input.as_str(),
			|b, s| b.iter(|| serde_json::from_str::<serde_json::Value>(black_box(s)).unwrap()),
		);

		group.finish();
	}
}

criterion_group!(
	benches,
	bench_parse,
	bench_parse_pathological,
	bench_parse_value
);
criterion_main!(benches);
