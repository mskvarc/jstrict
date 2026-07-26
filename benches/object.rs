use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use jstrict::{
	Object, Parse, Value,
	object::{Entry, Key},
};
use std::hint::black_box;

mod common;

fn build_unique_entries(n: usize) -> Vec<Entry> {
	(0..n)
		.map(|i| Entry::new(Key::from(format!("k{}", i)), Value::Number(i.into())))
		.collect()
}

fn build_dup_entries(n_unique: usize, n_dupes: usize) -> Vec<Entry> {
	let mut v = Vec::with_capacity(n_unique * n_dupes);
	for i in 0..n_unique {
		let key = Key::from(format!("k{}", i));
		for j in 0..n_dupes {
			v.push(Entry::new(key.clone(), Value::Number(j.into())));
		}
	}
	v
}

fn bench_insert(c: &mut Criterion) {
	let mut group = c.benchmark_group("object/insert_unique");
	for n in [16usize, 256, 4096] {
		let entries = build_unique_entries(n);
		group.bench_with_input(BenchmarkId::from_parameter(n), &entries, |b, entries| {
			b.iter(|| {
				let mut o = Object::new();
				for e in entries {
					o.insert(black_box(e.key.clone()), black_box(e.value.clone()));
				}
				o
			});
		});
	}
	group.finish();

	let mut group = c.benchmark_group("object/insert_with_dupes");
	for (uniq, dups) in [(16usize, 16usize), (64, 16), (256, 4)] {
		let entries = build_dup_entries(uniq, dups);
		let id = format!("{}u_x{}", uniq, dups);
		group.bench_with_input(BenchmarkId::from_parameter(id), &entries, |b, entries| {
			b.iter(|| {
				let mut o = Object::new();
				for e in entries {
					o.insert(black_box(e.key.clone()), black_box(e.value.clone()));
				}
				o
			});
		});
	}
	group.finish();
}

fn bench_from_vec(c: &mut Criterion) {
	let mut group = c.benchmark_group("object/from_vec");
	for n in [16usize, 256, 4096] {
		let entries = build_unique_entries(n);
		group.bench_with_input(BenchmarkId::from_parameter(n), &entries, |b, entries| {
			b.iter(|| Object::from_vec(black_box(entries.clone())));
		});
	}
	group.finish();
}

fn bench_get(c: &mut Criterion) {
	let json = common::duplicate_keys(64, 16);
	let value = Value::parse_str(&json).unwrap().0;
	let object = value.as_object().unwrap();

	let keys: Vec<String> = (0..64).map(|i| format!("k{}", i)).collect();

	c.bench_function("object/get_dup/64x16", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for k in &keys {
				n += object.get(black_box(k.as_str())).count();
			}
			n
		});
	});

	c.bench_function("object/get_unique_dup/64x16", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for k in &keys {
				if object.get_unique(black_box(k.as_str())).is_err() {
					n += 1;
				}
			}
			n
		});
	});

	let unique = build_unique_entries(4096);
	let unique_obj = Object::from_vec(unique);
	c.bench_function("object/get_unique_uniq/4k", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for i in 0..4096usize {
				let key = format!("k{}", i);
				if unique_obj
					.get_unique(black_box(key.as_str()))
					.unwrap()
					.is_some()
				{
					n += 1;
				}
			}
			n
		});
	});
}

fn bench_iter(c: &mut Criterion) {
	let entries = build_unique_entries(4096);
	let object = Object::from_vec(entries);
	c.bench_function("object/iter/4k", |b| {
		b.iter(|| {
			let mut n = 0usize;
			for entry in black_box(&object).iter() {
				if entry.value.is_number() {
					n += 1;
				}
			}
			n
		});
	});
}

fn bench_sort(c: &mut Criterion) {
	let entries = build_unique_entries(4096);
	c.bench_function("object/sort/4k", |b| {
		b.iter_batched(
			|| Object::from_vec(entries.clone()),
			|mut o| {
				o.sort();
				o
			},
			criterion::BatchSize::SmallInput,
		);
	});
}

criterion_group!(
	benches,
	bench_insert,
	bench_from_vec,
	bench_get,
	bench_iter,
	bench_sort
);
criterion_main!(benches);
