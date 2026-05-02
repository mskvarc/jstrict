use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use jstrict::{json, Value};

fn bench_macros(c: &mut Criterion) {
	c.bench_function("macros/simple", |b| {
		b.iter(|| {
			black_box(json!({
				"a": 1,
				"b": [1, 2, 3],
				"c": "hello",
				"d": null,
				"e": true,
			}))
		});
	});

	c.bench_function("macros/nested_5", |b| {
		b.iter(|| {
			black_box(json!({
				"l1": {
					"l2": {
						"l3": {
							"l4": {
								"l5": [1, 2, 3, 4, 5]
							}
						}
					}
				}
			}))
		});
	});

	c.bench_function("macros/interpolated", |b| {
		let id: u64 = 42;
		let name = "alice";
		let scores = [1.0_f64, 2.0, 3.0];
		b.iter(|| {
			let scores_v: Vec<Value> =
				scores.iter().map(|x| Value::try_from(*x).unwrap()).collect();
			black_box(json!({
				"id": Value::from(id),
				"name": Value::from(name),
				"scores": Value::Array(scores_v),
			}))
		});
	});
}

criterion_group!(benches, bench_macros);
criterion_main!(benches);
