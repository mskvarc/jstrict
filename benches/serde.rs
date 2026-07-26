use std::collections::HashMap;

use criterion::{Criterion, criterion_group, criterion_main};
use jstrict::{Object, Parse, Value};
use serde::{Deserialize, Serialize};
use std::hint::black_box;

mod common;

#[derive(Clone, Serialize, Deserialize)]
struct Address {
	street: String,
	city: String,
	zip: String,
	country: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
enum Status {
	Active,
	Suspended { reason: String },
	Deleted(u64),
}

#[derive(Clone, Serialize, Deserialize)]
struct User {
	id: u64,
	name: String,
	email: Option<String>,
	tags: Vec<String>,
	scores: Vec<f64>,
	address: Address,
	status: Status,
	attributes: HashMap<String, String>,
}

fn build_users(n: usize) -> Vec<User> {
	let mut rng = common::Rng::new(common::SEED);
	(0..n)
		.map(|i| User {
			id: i as u64,
			name: format!("user_{}", i),
			email: if rng.boolean() {
				Some(format!("u{}@example.com", i))
			} else {
				None
			},
			tags: (0..(rng.range(5))).map(|j| format!("tag_{}", j)).collect(),
			scores: (0..(rng.range(8)))
				.map(|_| rng.range(10_000) as f64 / 100.0)
				.collect(),
			address: Address {
				street: format!("{} Main St", rng.range(9999)),
				city: "Springfield".into(),
				zip: format!("{:05}", rng.range(99_999)),
				country: if rng.boolean() {
					Some("US".into())
				} else {
					None
				},
			},
			status: match rng.range(3) {
				0 => Status::Active,
				1 => Status::Suspended {
					reason: "policy".into(),
				},
				_ => Status::Deleted(rng.range(1_000_000)),
			},
			attributes: (0..(rng.range(4)))
				.map(|j| (format!("k{}", j), format!("v{}", j)))
				.collect(),
		})
		.collect()
}

fn bench_to_value(c: &mut Criterion) {
	let users = build_users(200);
	c.bench_function("serde/to_value/users_200", |b| {
		b.iter(|| jstrict::to_value(black_box(&users)).unwrap());
	});
	c.bench_function("serde/serde_json_to_value/users_200", |b| {
		b.iter(|| serde_json::to_value(black_box(&users)).unwrap());
	});
}

fn bench_from_value(c: &mut Criterion) {
	let users = build_users(200);
	let value = jstrict::to_value(&users).unwrap();
	let sj_value = serde_json::to_value(&users).unwrap();

	c.bench_function("serde/from_value/users_200", |b| {
		b.iter(|| {
			let v: Vec<User> = jstrict::from_value(black_box(value.clone())).unwrap();
			v
		});
	});
	c.bench_function("serde/serde_json_from_value/users_200", |b| {
		b.iter(|| {
			let v: Vec<User> = serde_json::from_value(black_box(sj_value.clone())).unwrap();
			v
		});
	});
}

fn bench_round_trip(c: &mut Criterion) {
	let users = build_users(200);
	c.bench_function("serde/round_trip/users_200", |b| {
		b.iter(|| {
			let v = jstrict::to_value(black_box(&users)).unwrap();
			let users2: Vec<User> = jstrict::from_value(v).unwrap();
			users2
		});
	});
}

fn bench_object_deserialize(c: &mut Criterion) {
	// Targets the `Object: Deserialize` impl added in commit a989e60.
	let json = common::wide_object(1_000);
	let value = Value::parse_str(&json).unwrap().0;
	c.bench_function("serde/object_deserialize/wide_1k", |b| {
		b.iter(|| {
			let o: Object = jstrict::from_value(black_box(value.clone())).unwrap();
			o
		});
	});
}

criterion_group!(
	benches,
	bench_to_value,
	bench_from_value,
	bench_round_trip,
	bench_object_deserialize
);
criterion_main!(benches);
