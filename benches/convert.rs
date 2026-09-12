use criterion::{Criterion, criterion_group, criterion_main};
use jstrict::Value;
use std::hint::black_box;

mod common;

fn pair(json: &str) -> (Value, serde_json::Value) {
	let ours = common::parsed(json).0;
	let sj: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
	(ours, sj)
}

fn bench_convert(c: &mut Criterion) {
	for (name, source) in [("medium", common::medium()), ("large", common::large())] {
		let (_, sj) = pair(&source);
		let jsn_again = common::parsed(&source).0;

		c.bench_function(&format!("convert/from_serde_json/{name}"), |b| {
			b.iter(|| Value::from_serde_json(black_box(sj.clone())));
		});

		c.bench_function(&format!("convert/into_serde_json/{name}"), |b| {
			b.iter(|| Value::into_serde_json(black_box(jsn_again.clone())));
		});

		c.bench_function(&format!("convert/round_trip/{name}"), |b| {
			b.iter(|| {
				let v = Value::from_serde_json(black_box(sj.clone()));
				Value::into_serde_json(v)
			});
		});
	}
}

criterion_group!(benches, bench_convert);
criterion_main!(benches);
