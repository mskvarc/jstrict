//! This example shows how to convert a `sonic_rs::Value` into/from a
//! `json_syntax::Value` using the `sonic-rs` feature.

fn main() {
	// First we create a `sonic_rs` value.
	let a = sonic_rs::json!({
		"foo": 1,
		"bar": [2, 3]
	});

	// We convert the `sonic_rs` value into a `json_syntax` value.
	let b = json_syntax::Value::from_sonic_rs(a);

	// We convert it back into a `sonic_rs` value.
	let _ = json_syntax::Value::into_sonic_rs(b);
}
