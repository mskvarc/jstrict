use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use jstrict::{parse::Options, Parse, Value};

mod common;

fn quoted(content: &str) -> String {
	let mut s = String::with_capacity(content.len() + 2);
	s.push('"');
	s.push_str(content);
	s.push('"');
	s
}

fn cases() -> Vec<(&'static str, String)> {
	let ascii_inline: String = "abcdefghijkl".into();
	let ascii_spill: String = (0..200).map(|i| (b'a' + (i % 26) as u8) as char).collect();
	let escapes_quote: String = "\\\"".repeat(256);
	let backslashes: String = "\\\\".repeat(256);
	let bmp_escapes: String = "\\u00ff".repeat(256);
	let mut pathological = String::with_capacity(100_000);
	for i in 0..6_250 {
		// 16 chars between escapes; total ~100KB.
		for _ in 0..16 {
			pathological.push((b'a' + (i % 26) as u8) as char);
		}
		pathological.push_str("\\n");
	}

	vec![
		("ascii_inline", quoted(&ascii_inline)),
		("ascii_spill", quoted(&ascii_spill)),
		("escape_quote_heavy", quoted(&escapes_quote)),
		("escape_backslash_heavy", quoted(&backslashes)),
		("bmp_escape_heavy", quoted(&bmp_escapes)),
		("surrogate_pair_heavy", common::surrogate_heavy(1_000)),
		("pathological_100kb", quoted(&pathological)),
		(
			"long_ascii_100kb",
			common::long_ascii_string(100_000),
		),
	]
}

fn bench_strings(c: &mut Criterion) {
	for (name, source) in cases() {
		c.bench_function(&format!("strings/{}", name), |b| {
			b.iter(|| Value::parse_str(black_box(source.as_str())).unwrap());
		});
	}

	let surrogates = common::surrogate_heavy(1_000);
	let flex = Options::flexible();
	c.bench_function("strings/surrogate_pair_flexible", |b| {
		b.iter(|| Value::parse_str_with(black_box(surrogates.as_str()), flex).unwrap());
	});
}

criterion_group!(benches, bench_strings);
criterion_main!(benches);
