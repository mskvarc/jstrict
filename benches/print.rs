use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use jstrict::{
	Parse, Print, Value,
	print::{Indent, Limit, Options as PrintOptions},
};
use std::hint::black_box;

mod common;

fn fixtures() -> Vec<(&'static str, String)> {
	vec![
		("tiny", common::tiny()),
		("small", common::small()),
		("medium", common::medium()),
		("wide_object_1k", common::wide_object(1_000)),
		("number_heavy_1k", common::number_heavy(1_000)),
		("string_heavy_1k", common::string_heavy(1_000, 0.25)),
	]
}

const fn custom_options() -> PrintOptions {
	let mut opts = PrintOptions::pretty();
	opts.indent = Indent::Tabs(1);
	opts.array_limit = Some(Limit::ItemOrWidth(2, 32));
	opts.object_limit = Some(Limit::ItemOrWidth(2, 32));
	opts
}

fn bench_print(c: &mut Criterion) {
	for (name, input) in fixtures() {
		let value = Value::parse_str(&input).unwrap().0;
		let sj_value: serde_json::Value = serde_json::from_str(&input).unwrap();

		let compact_size = value.compact_print().to_string().len() as u64;
		let pretty_size = value.pretty_print().to_string().len() as u64;

		let mut group = c.benchmark_group(format!("print/{}", name));

		group.throughput(Throughput::Bytes(compact_size));
		group.bench_with_input(BenchmarkId::new("jstrict", "compact"), &value, |b, v| {
			b.iter(|| black_box(v).compact_print().to_string());
		});
		group.bench_with_input(BenchmarkId::new("jstrict", "inline"), &value, |b, v| {
			b.iter(|| black_box(v).inline_print().to_string());
		});

		group.throughput(Throughput::Bytes(pretty_size));
		group.bench_with_input(BenchmarkId::new("jstrict", "pretty"), &value, |b, v| {
			b.iter(|| black_box(v).pretty_print().to_string());
		});
		let custom = custom_options();
		group.bench_with_input(BenchmarkId::new("jstrict", "custom"), &value, |b, v| {
			b.iter(|| black_box(v).print_with(custom.clone()).to_string());
		});

		group.throughput(Throughput::Bytes(compact_size));
		group.bench_with_input(
			BenchmarkId::new("serde_json", "to_string"),
			&sj_value,
			|b, v| {
				b.iter(|| serde_json::to_string(black_box(v)).unwrap());
			},
		);
		group.throughput(Throughput::Bytes(pretty_size));
		group.bench_with_input(
			BenchmarkId::new("serde_json", "to_string_pretty"),
			&sj_value,
			|b, v| {
				b.iter(|| serde_json::to_string_pretty(black_box(v)).unwrap());
			},
		);

		group.finish();
	}
}

criterion_group!(benches, bench_print);
criterion_main!(benches);
