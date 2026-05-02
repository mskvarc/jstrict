use jstrict::{json, Parse, Print};

#[test]
fn print_01() {
	let value = json! { null };
	assert_eq!(value.pretty_print().to_string(), "null")
}

#[test]
fn print_02() {
	let value = json! { true };
	assert_eq!(value.pretty_print().to_string(), "true")
}

#[test]
fn print_03() {
	let value = json! { false };
	assert_eq!(value.pretty_print().to_string(), "false")
}

#[test]
fn print_04() {
	let value = json! { "foo" };
	assert_eq!(value.pretty_print().to_string(), "\"foo\"")
}

#[test]
fn print_05() {
	let value = json! { 1 };
	assert_eq!(value.pretty_print().to_string(), "1")
}

#[test]
fn print_06() {
	let value = json! { [] };
	assert_eq!(value.pretty_print().to_string(), "[]")
}

#[test]
fn print_07() {
	let value = json! { [ null ] };
	assert_eq!(value.pretty_print().to_string(), "[ null ]")
}

#[test]
fn print_08() {
	let value = json! { [ "azertyuiop" ] };
	assert_eq!(value.pretty_print().to_string(), "[ \"azertyuiop\" ]")
}

#[test]
fn print_09() {
	let value = json! { [ "azertyuiopq" ] };
	assert_eq!(value.pretty_print().to_string(), "[\n  \"azertyuiopq\"\n]")
}

#[test]
fn print_10() {
	let value = json! { [ true, false ] };
	assert_eq!(value.pretty_print().to_string(), "[\n  true,\n  false\n]")
}

#[test]
fn print_11() {
	let value = json! { { "a": null } };
	assert_eq!(value.pretty_print().to_string(), "{ \"a\": null }")
}

#[test]
fn print_12() {
	let value = json! { { "a": null, "b": 12 } };
	assert_eq!(
		value.pretty_print().to_string(),
		"{\n  \"a\": null,\n  \"b\": 12\n}"
	)
}

#[test]
fn print_13() {
	let value = json! { { "a": [ null ], "b": [ 13 ] } };
	assert_eq!(
		value.pretty_print().to_string(),
		"{\n  \"a\": [ null ],\n  \"b\": [ 13 ]\n}"
	)
}

#[test]
fn print_14() {
	let value = json! { { "a": [ null, [] ], "b": [ 14 ] } };
	assert_eq!(
		value.pretty_print().to_string(),
		"{\n  \"a\": [\n    null,\n    []\n  ],\n  \"b\": [ 14 ]\n}"
	)
}

fn manual_escape(s: &str) -> String {
	let mut out = String::from("\"");
	let bytes = s.as_bytes();
	let mut i = 0;
	while i < bytes.len() {
		let b = bytes[i];
		match b {
			b'\\' => {
				out.push_str("\\\\");
				i += 1;
			}
			b'"' => {
				out.push_str("\\\"");
				i += 1;
			}
			0x08 => {
				out.push_str("\\b");
				i += 1;
			}
			0x09 => {
				out.push_str("\\t");
				i += 1;
			}
			0x0a => {
				out.push_str("\\n");
				i += 1;
			}
			0x0c => {
				out.push_str("\\f");
				i += 1;
			}
			0x0d => {
				out.push_str("\\r");
				i += 1;
			}
			c if c < 0x20 => {
				out.push_str(&format!("\\u00{:02x}", c));
				i += 1;
			}
			_ => {
				// UTF-8 leading byte: copy the whole code point.
				// 0x80..0xc0 shouldn't occur in valid UTF-8 leading position; treat as 1.
				let n = if b < 0xc0 {
					1
				} else if b < 0xe0 {
					2
				} else if b < 0xf0 {
					3
				} else {
					4
				};
				out.push_str(std::str::from_utf8(&bytes[i..i + n]).unwrap());
				i += n;
			}
		}
	}
	out.push('"');
	out
}

#[test]
fn print_simd_all_controls() {
	// Build a string longer than the SIMD threshold (64) that touches every
	// control byte 0x00..=0x1f, the short-form bytes, `"`, `\`, and a
	// multi-byte UTF-8 sequence.
	let mut s = String::new();
	for b in 0u8..=0x1f {
		s.push(b as char);
	}
	s.push('"');
	s.push('\\');
	s.push('/');
	s.push_str("ünîcødé"); // multi-byte UTF-8
	s.push_str("padding to push past SIMD threshold padding padding padding");
	assert!(s.len() >= 64);

	let value = jstrict::Value::String(s.clone().into());
	let printed = value.compact_print().to_string();
	let expected = manual_escape(&s);
	assert_eq!(printed, expected);

	// Round-trip.
	let (parsed, _) = jstrict::Value::parse_str(&printed).unwrap();
	assert_eq!(parsed, value);
}

#[test]
fn print_simd_threshold_boundary() {
	// Lengths 63 (SWAR path) and 64 (SIMD path) must produce identical
	// output for the same content.
	let base = "abc\nXYZ\\\"def"; // forces escapes early
	let pad = "x".repeat(63 - base.len());
	let s63: String = format!("{}{}", base, pad);
	let s64: String = format!("{}{}", s63, "x");
	assert_eq!(s63.len(), 63);
	assert_eq!(s64.len(), 64);

	let p63 = jstrict::Value::String(s63.clone().into())
		.compact_print()
		.to_string();
	let p64 = jstrict::Value::String(s64.clone().into())
		.compact_print()
		.to_string();

	assert_eq!(p63, manual_escape(&s63));
	assert_eq!(p64, manual_escape(&s64));
}
