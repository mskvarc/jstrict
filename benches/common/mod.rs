#![allow(dead_code)]

use json_syntax::{CodeMap, Parse, Value};
use std::fmt::Write as _;

pub const SEED: u64 = 0xC0FFEE;

pub struct Rng(u64);

impl Rng {
	pub fn new(seed: u64) -> Self {
		Self(if seed == 0 { 1 } else { seed })
	}

	pub fn next(&mut self) -> u64 {
		let mut x = self.0;
		x ^= x << 13;
		x ^= x >> 7;
		x ^= x << 17;
		self.0 = x;
		x
	}

	pub fn range(&mut self, n: u64) -> u64 {
		self.next() % n
	}

	pub fn boolean(&mut self) -> bool {
		self.next() & 1 == 1
	}

	pub fn unit(&mut self) -> f32 {
		(self.next() >> 16) as f32 / (1u64 << 48) as f32
	}
}

fn push_word(s: &mut String, rng: &mut Rng, min_len: u64, max_len: u64) {
	let len = rng.range(max_len - min_len) + min_len;
	for _ in 0..len {
		let c = (b'a' + (rng.range(26)) as u8) as char;
		s.push(c);
	}
}

fn write_record(s: &mut String, rng: &mut Rng) {
	let id = rng.next();
	let active = rng.boolean();
	let score = (rng.range(1_000_000)) as f64 / 1000.0;
	let tags_n = rng.range(5) as usize;
	write!(s, r#"{{"id":{},"active":{},"score":{},"name":""#, id, active, score).unwrap();
	push_word(s, rng, 4, 14);
	s.push_str(r#"","tags":["#);
	for i in 0..tags_n {
		if i > 0 {
			s.push(',');
		}
		s.push('"');
		push_word(s, rng, 3, 10);
		s.push('"');
	}
	s.push_str(r#"],"nullable":"#);
	if rng.boolean() {
		write!(s, "{}", rng.range(1000)).unwrap();
	} else {
		s.push_str("null");
	}
	s.push_str(r#","nested":{"a":"#);
	write!(s, "{}", rng.range(1_000_000) as i64 - 500_000).unwrap();
	s.push_str(r#","b":["#);
	let inner_n = rng.range(4) as usize;
	for i in 0..inner_n {
		if i > 0 {
			s.push(',');
		}
		write!(s, "{}", rng.range(10_000)).unwrap();
	}
	s.push_str("]}}");
}

/// Mixed realistic JSON of approximately `target_bytes` size.
pub fn mixed(target_bytes: usize) -> String {
	let mut rng = Rng::new(SEED);
	let mut s = String::with_capacity(target_bytes + 128);
	s.push_str(
		r#"{"meta":{"version":1,"generated_at":"2026-05-01T00:00:00Z"},"records":["#,
	);
	let mut first = true;
	while s.len() < target_bytes {
		if !first {
			s.push(',');
		}
		first = false;
		write_record(&mut s, &mut rng);
	}
	s.push_str("]}");
	s
}

pub fn tiny() -> String {
	mixed(80)
}

pub fn small() -> String {
	mixed(10_000)
}

pub fn medium() -> String {
	mixed(100_000)
}

pub fn large() -> String {
	mixed(1_000_000)
}

pub fn deep_array(depth: usize) -> String {
	let mut s = String::with_capacity(depth * 2 + 8);
	for _ in 0..depth {
		s.push('[');
	}
	s.push('0');
	for _ in 0..depth {
		s.push(']');
	}
	s
}

pub fn deep_object(depth: usize) -> String {
	let mut s = String::with_capacity(depth * 8 + 8);
	for _ in 0..depth {
		s.push_str(r#"{"k":"#);
	}
	s.push('0');
	for _ in 0..depth {
		s.push('}');
	}
	s
}

pub fn wide_object(n_keys: usize) -> String {
	let mut s = String::with_capacity(n_keys * 16);
	s.push('{');
	for i in 0..n_keys {
		if i > 0 {
			s.push(',');
		}
		write!(s, r#""key_{}":{}"#, i, i).unwrap();
	}
	s.push('}');
	s
}

pub fn wide_array(n_items: usize) -> String {
	let mut s = String::with_capacity(n_items * 8);
	s.push('[');
	for i in 0..n_items {
		if i > 0 {
			s.push(',');
		}
		write!(s, "{}", i).unwrap();
	}
	s.push(']');
	s
}

pub fn number_heavy(n: usize) -> String {
	let mut rng = Rng::new(SEED);
	let mut s = String::with_capacity(n * 24);
	s.push('[');
	for i in 0..n {
		if i > 0 {
			s.push(',');
		}
		match i % 5 {
			0 => write!(s, "{}", rng.next() as i64).unwrap(),
			1 => write!(s, "{}", -(rng.range(1_000_000) as i64)).unwrap(),
			2 => write!(s, "{:.6}", rng.range(1_000_000) as f64 / 1000.0).unwrap(),
			3 => write!(
				s,
				"1.{}e{}",
				rng.range(1_000_000),
				rng.range(60) as i32 - 30
			)
			.unwrap(),
			_ => {
				s.push('1');
				for _ in 0..49 {
					write!(s, "{}", rng.range(10)).unwrap();
				}
			}
		}
	}
	s.push(']');
	s
}

pub fn string_heavy(n: usize, escape_density: f32) -> String {
	let mut rng = Rng::new(SEED);
	let mut s = String::with_capacity(n * 32);
	s.push('[');
	for i in 0..n {
		if i > 0 {
			s.push(',');
		}
		s.push('"');
		let len = rng.range(32) as usize + 16;
		for _ in 0..len {
			if rng.unit() < escape_density {
				match rng.range(5) {
					0 => s.push_str("\\\""),
					1 => s.push_str("\\\\"),
					2 => s.push_str("\\n"),
					3 => s.push_str("\\u00ff"),
					_ => s.push_str("\\u20ac"),
				}
			} else {
				let c = (b'a' + rng.range(26) as u8) as char;
				s.push(c);
			}
		}
		s.push('"');
	}
	s.push(']');
	s
}

pub fn duplicate_keys(n_unique: usize, n_dupes_per_key: usize) -> String {
	let mut s = String::new();
	s.push('{');
	let mut first = true;
	for i in 0..n_unique {
		for j in 0..n_dupes_per_key {
			if !first {
				s.push(',');
			}
			first = false;
			write!(s, r#""k{}":{}"#, i, j).unwrap();
		}
	}
	s.push('}');
	s
}

/// Pathological surrogate-pair-heavy string. Each pair encodes U+1D11E (G clef).
pub fn surrogate_heavy(n_pairs: usize) -> String {
	let mut s = String::with_capacity(n_pairs * 12 + 2);
	s.push('"');
	for _ in 0..n_pairs {
		s.push_str("\\uD834\\uDD1E");
	}
	s.push('"');
	s
}

/// Single number literal of `digits` digits.
pub fn huge_number(digits: usize) -> String {
	let mut rng = Rng::new(SEED);
	let mut s = String::with_capacity(digits + 1);
	s.push('1');
	for _ in 1..digits {
		s.push((b'0' + rng.range(10) as u8) as char);
	}
	s
}

/// Long ASCII string literal that exceeds SmallStr inline cap.
pub fn long_ascii_string(byte_len: usize) -> String {
	let mut rng = Rng::new(SEED);
	let mut s = String::with_capacity(byte_len + 2);
	s.push('"');
	for _ in 0..byte_len {
		s.push((b'a' + rng.range(26) as u8) as char);
	}
	s.push('"');
	s
}

pub fn parsed(json: &str) -> (Value, CodeMap) {
	Value::parse_str(json).expect("fixture parse error")
}
