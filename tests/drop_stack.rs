//! Dropping deeply nested values never recurses.
//!
//! Every fixture is built with a loop and dropped on a thread whose stack is
//! far too small for a recursive drop of its depth.
use jstrict::parse::parse_str_value;
use jstrict::{Array, Object, Value, json, object::Entry};
use std::thread;

/// Nesting depth of every deep fixture.
const DEPTH: usize = 100_000;

/// Stack of the thread each deep value is dropped on.
const SMALL_STACK: usize = 64 * 1024;

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

/// `[[[ ... null ... ]]]`, `depth` arrays deep.
fn deep_array(depth: usize) -> Value {
	let mut value = Value::Null;
	for _ in 0..depth {
		value = Value::Array(vec![value].into());
	}
	value
}

/// `{"a": {"a": ... null ...}}`, `depth` objects deep.
fn deep_object_chain(depth: usize) -> Value {
	let mut value = Value::Null;
	for _ in 0..depth {
		value = Value::Object(Object::from_vec(vec![Entry::new("a".into(), value)]));
	}
	value
}

#[test]
fn dropping_a_100k_deep_array_on_a_64_kib_thread_returns() {
	let value = deep_array(DEPTH);
	on_small_stack(move || drop(value));
}

#[test]
fn dropping_a_100k_deep_object_chain_on_a_64_kib_thread_returns() {
	let value = deep_object_chain(DEPTH);
	on_small_stack(move || drop(value));
}

#[test]
fn dropping_100k_alternating_arrays_and_objects_on_a_64_kib_thread_returns() {
	let mut value = Value::Null;
	for level in 0..DEPTH {
		value = if level % 2 == 0 {
			Value::Array(vec![Value::Boolean(true), value].into())
		} else {
			Value::Object(Object::from_vec(vec![
				Entry::new("k".into(), value),
				Entry::new("n".into(), Value::Null),
			]))
		};
	}
	on_small_stack(move || drop(value));
}

#[test]
fn parsing_then_dropping_a_100k_deep_document_on_a_64_kib_thread_returns() {
	let text = format!("{}{}", "[".repeat(DEPTH), "]".repeat(DEPTH));
	on_small_stack(move || {
		let value = parse_str_value(&text).expect("the document parses");
		assert!(value.is_array());
	});
}

#[test]
fn a_syntax_error_100k_levels_deep_is_reported_on_a_64_kib_thread() {
	let text = format!("{}{}", "[{\"a\":".repeat(DEPTH), "}");
	let outcome = on_small_stack(move || parse_str_value(&text).is_err());
	assert!(outcome, "an unterminated document is a syntax error");
}

#[test]
fn object_into_iter_still_yields_every_entry_in_order_including_duplicates() {
	let object = Object::from_vec(vec![
		Entry::new("a".into(), json!(1)),
		Entry::new("b".into(), json!([2])),
		Entry::new("a".into(), json!({"c": 3})),
	]);

	let entries: Vec<(String, Value)> = object
		.into_iter()
		.map(|entry| (entry.key.to_string(), entry.value))
		.collect();

	assert_eq!(
		entries,
		vec![
			("a".to_owned(), json!(1)),
			("b".to_owned(), json!([2])),
			("a".to_owned(), json!({"c": 3}))
		]
	);
}

#[test]
fn array_into_iter_and_into_vec_yield_every_item_in_order() {
	let array: Array = vec![json!(1), json!([2]), json!({"c": 3})].into();
	let copy = array.clone();

	let iterated: Vec<Value> = array.into_iter().collect();
	let vector = copy.into_vec();

	assert_eq!(iterated, vec![json!(1), json!([2]), json!({"c": 3})]);
	assert_eq!(vector, iterated);
}

#[test]
fn a_value_matched_by_value_moves_its_array_and_object_out() {
	let items = match json!([1, 2]) {
		Value::Array(items) => items,
		other @ (Value::Null
		| Value::Boolean(_)
		| Value::Number(_)
		| Value::String(_)
		| Value::Object(_)) => {
			panic!("an array, got {other:?}")
		}
	};
	let object = match json!({"a": 1}) {
		Value::Object(object) => object,
		other @ (Value::Null
		| Value::Boolean(_)
		| Value::Number(_)
		| Value::String(_)
		| Value::Array(_)) => {
			panic!("an object, got {other:?}")
		}
	};

	assert_eq!(items.len(), 2);
	assert_eq!(object.len(), 1);
}
