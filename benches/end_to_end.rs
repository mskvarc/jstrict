use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use std::hint::black_box;
use jstrict::{Parse, Print, Value};

mod common;

fn bench_parse_traverse_print(c: &mut Criterion) {
	let medium = common::medium();
	let mut group = c.benchmark_group("end_to_end/parse_traverse_compact");
	group.throughput(Throughput::Bytes(medium.len() as u64));
	group.bench_function("medium", |b| {
		b.iter(|| {
			let v = Value::parse_str(black_box(medium.as_str())).unwrap().0;
			let count = v.traverse().count();
			let printed = v.compact_print().to_string();
			(count, printed.len())
		});
	});
	group.finish();
}

#[cfg(feature = "canonicalize")]
fn bench_parse_canonicalize_print(c: &mut Criterion) {
	let medium = common::medium();
	let mut group = c.benchmark_group("end_to_end/parse_canonicalize_compact");
	group.throughput(Throughput::Bytes(medium.len() as u64));
	group.bench_function("medium", |b| {
		b.iter(|| {
			let mut v = Value::parse_str(black_box(medium.as_str())).unwrap().0;
			v.canonicalize();
			v.compact_print().to_string()
		});
	});
	group.finish();
}

#[cfg(feature = "serde")]
fn bench_serde_round_trip(c: &mut Criterion) {
	use serde::{Deserialize, Serialize};

	#[derive(Serialize, Deserialize)]
	struct Record {
		id: u64,
		name: String,
		tags: Vec<String>,
	}

	let records: Vec<Record> = (0..100)
		.map(|i| Record {
			id: i,
			name: format!("name_{}", i),
			tags: vec!["a".into(), "b".into(), "c".into()],
		})
		.collect();
	let value = jstrict::to_value(&records).unwrap();
	let printed = value.compact_print().to_string();

	c.bench_function("end_to_end/parse_from_to_print/records_100", |b| {
		b.iter(|| {
			let v = Value::parse_str(black_box(printed.as_str())).unwrap().0;
			let r: Vec<Record> = jstrict::from_value(v).unwrap();
			let v2 = jstrict::to_value(&r).unwrap();
			v2.compact_print().to_string()
		});
	});
}

#[cfg(feature = "serde_json")]
fn bench_serde_json_modify(c: &mut Criterion) {
	let medium = common::medium();
	let sj: serde_json::Value = serde_json::from_str(&medium).unwrap();
	c.bench_function("end_to_end/serde_json_modify", |b| {
		b.iter(|| {
			let mut v = Value::from_serde_json(black_box(sj.clone()));
			if let Some(o) = v.as_object_mut() {
				o.insert("touched".into(), Value::Boolean(true));
			}
			Value::into_serde_json(v)
		});
	});
}

#[cfg(all(not(feature = "canonicalize"), not(feature = "serde"), not(feature = "serde_json")))]
criterion_group!(benches, bench_parse_traverse_print);

#[cfg(all(feature = "canonicalize", not(feature = "serde"), not(feature = "serde_json")))]
criterion_group!(benches, bench_parse_traverse_print, bench_parse_canonicalize_print);

#[cfg(all(not(feature = "canonicalize"), feature = "serde", not(feature = "serde_json")))]
criterion_group!(benches, bench_parse_traverse_print, bench_serde_round_trip);

#[cfg(all(not(feature = "canonicalize"), not(feature = "serde"), feature = "serde_json"))]
criterion_group!(benches, bench_parse_traverse_print, bench_serde_json_modify);

#[cfg(all(feature = "canonicalize", feature = "serde", not(feature = "serde_json")))]
criterion_group!(
	benches,
	bench_parse_traverse_print,
	bench_parse_canonicalize_print,
	bench_serde_round_trip
);

#[cfg(all(feature = "canonicalize", not(feature = "serde"), feature = "serde_json"))]
criterion_group!(
	benches,
	bench_parse_traverse_print,
	bench_parse_canonicalize_print,
	bench_serde_json_modify
);

#[cfg(all(not(feature = "canonicalize"), feature = "serde", feature = "serde_json"))]
criterion_group!(
	benches,
	bench_parse_traverse_print,
	bench_serde_round_trip,
	bench_serde_json_modify
);

#[cfg(all(feature = "canonicalize", feature = "serde", feature = "serde_json"))]
criterion_group!(
	benches,
	bench_parse_traverse_print,
	bench_parse_canonicalize_print,
	bench_serde_round_trip,
	bench_serde_json_modify
);

criterion_main!(benches);
