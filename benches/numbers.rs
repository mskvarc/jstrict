use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use jstrict::{Parse, Value};

mod common;

fn cases() -> Vec<(&'static str, String)> {
	vec![
		("small_i64", "12345".into()),
		("large_i64", "9223372036854775807".into()),
		("negative", "-1234567890".into()),
		("f64_short", "3.14".into()),
		("f64_long", "3.141592653589793238462643383279".into()),
		("scientific", "1.23e-45".into()),
		(
			"arbitrary_int_200",
			{
				let mut s = String::with_capacity(200);
				let mut rng = common::Rng::new(common::SEED);
				s.push('1');
				for _ in 0..199 {
					s.push((b'0' + rng.range(10) as u8) as char);
				}
				s
			},
		),
		(
			"arbitrary_decimal_200",
			{
				let mut s = String::with_capacity(202);
				let mut rng = common::Rng::new(common::SEED);
				s.push('1');
				s.push('.');
				for _ in 0..200 {
					s.push((b'0' + rng.range(10) as u8) as char);
				}
				s
			},
		),
		("worst_case_1kb", common::huge_number(1024)),
	]
}

fn bench_numbers(c: &mut Criterion) {
	for (name, source) in cases() {
		c.bench_function(&format!("numbers/{}", name), |b| {
			b.iter(|| Value::parse_str(black_box(source.as_str())).unwrap());
		});
	}
}

criterion_group!(benches, bench_numbers);
criterion_main!(benches);
