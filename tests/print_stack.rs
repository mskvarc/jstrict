//! Printing deeply nested values on a small stack.
//!
//! Every fixture is built with a loop and printed on a thread whose stack is
//! far too small for an unguarded recursive printer of its depth.
use jstrict::parse::parse_str_value;
use jstrict::{Object, Print, Value, json, object::Entry};
use std::{hint::black_box, thread};

/// Nesting depth of every deep fixture.
const DEPTH: usize = 100_000;

/// Nesting depth of the pretty-printed fixture: pretty printing indents each
/// level, so its output grows with the square of the depth.
const PRETTY_DEPTH: usize = 10_000;

/// Stack of the thread each deep value is printed on.
const SMALL_STACK: usize = 256 * 1024;

/// Runs `job` on a thread with a [`SMALL_STACK`] and waits for it.
#[cfg(test)]
fn on_small_stack<R: Send + 'static>(job: impl FnOnce() -> R + Send + 'static) -> R {
	thread::Builder::new()
		.stack_size(SMALL_STACK)
		.spawn(job)
		.expect("the thread starts")
		.join()
		.expect("the thread returns instead of overflowing its stack")
}

/// `[[[ ... ]]]`, `depth` arrays deep, as text.
fn deep_array_text(depth: usize) -> String {
	format!("{}{}", "[".repeat(depth), "]".repeat(depth))
}

/// `{"a": {"a": ... null ...}}`, `depth` objects deep.
fn deep_object_chain(depth: usize) -> Value {
	let mut value = Value::Null;
	for _ in 0..depth {
		value = Value::Object(Object::from_vec(vec![Entry::new("a".into(), value)]));
	}
	value
}

/// Runs `job` with `bytes` of the current stack in use.
#[cfg(test)]
fn used_stack<R>(bytes: usize, job: impl FnOnce() -> R) -> R {
	/// Stack used by each frame of the descent.
	const FRAME: usize = 8 * 1024;
	let ballast = [0_u8; FRAME];
	black_box(&ballast);
	if bytes <= FRAME {
		job()
	} else {
		black_box(used_stack(bytes - FRAME, job))
	}
}

#[test]
fn compact_printing_a_100k_deep_array_on_a_256_kib_thread_reproduces_the_input_text() {
	let text = deep_array_text(DEPTH);
	let value = parse_str_value(&text).expect("the document parses");
	let printed = on_small_stack(move || value.compact_print().to_string());
	assert!(printed == text, "the compact print reproduces the input");
}

#[test]
fn pretty_printing_a_10k_deep_object_chain_on_a_256_kib_thread_succeeds() {
	let value = deep_object_chain(PRETTY_DEPTH);
	let printed = on_small_stack(move || value.pretty_print().to_string());
	assert!(printed.starts_with("{\n"));
	assert!(printed.contains("null"));
}

#[test]
fn display_of_a_100k_deep_value_on_a_nearly_exhausted_stack_succeeds() {
	let value = deep_object_chain(DEPTH);
	let printed = on_small_stack(move || {
		// Use most of the thread's stack before printing.
		used_stack(160 * 1024, || value.to_string())
	});
	assert_eq!(printed.len(), DEPTH * "{\"a\":}".len() + "null".len());
}

#[cfg(feature = "serde")]
#[test]
fn serializing_a_100k_deep_value_through_serde_json_to_writer_on_a_256_kib_thread_succeeds() {
	let text = deep_array_text(DEPTH);
	let value = parse_str_value(&text).expect("the document parses");
	let written = on_small_stack(move || {
		let mut written = Vec::new();
		serde_json::to_writer(&mut written, &value).expect("the value serializes");
		written
	});
	assert!(
		written == text.as_bytes(),
		"serialization reproduces the input"
	);
}

#[test]
fn printing_a_shallow_value_matches_the_output_before_the_guard() {
	let value = json!({"a": [1, 2, {"b": null}], "c": "d"});
	assert_eq!(
		value.compact_print().to_string(),
		r#"{"a":[1,2,{"b":null}],"c":"d"}"#
	);
	assert_eq!(
		value.inline_print().to_string(),
		r#"{ "a": [ 1, 2, { "b": null } ], "c": "d" }"#
	);
}
