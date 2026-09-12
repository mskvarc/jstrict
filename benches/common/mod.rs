#![allow(
	dead_code,
	reason = "each benchmark binary pulls in this shared module and uses a different subset of the generators"
)]

use jstrict::{CodeMap, Parse, Value};
use std::fmt::Write as _;

pub const SEED: u64 = 0x00C0_FFEE;

pub struct Rng(u64);

impl Rng {
	pub const fn new(seed: u64) -> Self {
		Self(if seed == 0 { 1 } else { seed })
	}

	pub const fn next(&mut self) -> u64 {
		let mut x = self.0;
		x ^= x << 13;
		x ^= x >> 7;
		x ^= x << 17;
		self.0 = x;
		x
	}

	pub const fn range(&mut self, n: u64) -> u64 {
		self.next() % n
	}

	pub const fn boolean(&mut self) -> bool {
		self.next() & 1 == 1
	}

	pub fn unit(&mut self) -> f32 {
		// 16 bits of randomness map losslessly into an `f32`.
		let bits = u16::try_from(self.next() >> 48).unwrap_or(0);
		f32::from(bits) / f32::from(u16::MAX)
	}
}

fn ascii_letter(rng: &mut Rng) -> char {
	(b'a' + u8::try_from(rng.range(26)).unwrap_or(0)) as char
}

fn push_word(s: &mut String, rng: &mut Rng, min_len: u64, max_len: u64) {
	let len = rng.range(max_len - min_len) + min_len;
	for _ in 0..len {
		s.push(ascii_letter(rng));
	}
}

fn write_record(s: &mut String, rng: &mut Rng) {
	let id = rng.next();
	let active = rng.boolean();
	let score = f64::from(u32::try_from(rng.range(1_000_000)).unwrap_or(0)) / 1000.0;
	let tags_n = usize::try_from(rng.range(5)).unwrap_or(0);
	let _ = write!(
		s,
		r#"{{"id":{id},"active":{active},"score":{score},"name":""#
	);
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
		let _ = write!(s, "{}", rng.range(1000));
	} else {
		s.push_str("null");
	}
	s.push_str(r#","nested":{"a":"#);
	let _ = write!(
		s,
		"{}",
		i64::try_from(rng.range(1_000_000)).unwrap_or(0) - 500_000
	);
	s.push_str(r#","b":["#);
	let inner_n = usize::try_from(rng.range(4)).unwrap_or(0);
	for i in 0..inner_n {
		if i > 0 {
			s.push(',');
		}
		let _ = write!(s, "{}", rng.range(10_000));
	}
	s.push_str("]}}");
}

/// Mixed realistic JSON of approximately `target_bytes` size.
pub fn mixed(target_bytes: usize) -> String {
	let mut rng = Rng::new(SEED);
	let mut s = String::with_capacity(target_bytes + 128);
	s.push_str(r#"{"meta":{"version":1,"generated_at":"2026-05-01T00:00:00Z"},"records":["#);
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
		let _ = write!(s, r#""key_{i}":{i}"#);
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
		let _ = write!(s, "{i}");
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
			0 => {
				let _ = write!(s, "{}", i64::from_ne_bytes(rng.next().to_ne_bytes()));
			}
			1 => {
				let _ = write!(s, "{}", -(i64::try_from(rng.range(1_000_000)).unwrap_or(0)));
			}
			2 => {
				let value = f64::from(u32::try_from(rng.range(1_000_000)).unwrap_or(0)) / 1000.0;
				let _ = write!(s, "{value:.6}");
			}
			3 => {
				let exp = i32::try_from(rng.range(60)).unwrap_or(0) - 30;
				let _ = write!(s, "1.{}e{exp}", rng.range(1_000_000));
			}
			_ => {
				s.push('1');
				for _ in 0..49 {
					let _ = write!(s, "{}", rng.range(10));
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
		let len = usize::try_from(rng.range(32)).unwrap_or(0) + 16;
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
				s.push(ascii_letter(&mut rng));
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
			let _ = write!(s, r#""k{i}":{j}"#);
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
		s.push((b'0' + u8::try_from(rng.range(10)).unwrap_or(0)) as char);
	}
	s
}

/// Long ASCII string literal that exceeds the inline string capacity.
pub fn long_ascii_string(byte_len: usize) -> String {
	let mut rng = Rng::new(SEED);
	let mut s = String::with_capacity(byte_len + 2);
	s.push('"');
	for _ in 0..byte_len {
		s.push(ascii_letter(&mut rng));
	}
	s.push('"');
	s
}

pub fn parsed(json: &str) -> (Value, CodeMap) {
	Value::parse_str(json).unwrap_or_else(|_| (Value::Null, CodeMap::default()))
}
