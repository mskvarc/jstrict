//! This example shows how to convert a `sonic_rs::Value` into/from a
//! `jstrict::Value` using the `sonic-rs` feature.

fn main() {
	// First we create a `sonic_rs` value.
	let a = sonic_rs::json!({
		"foo": 1,
		"bar": [2, 3]
	});

	// We convert the `sonic_rs` value into a `jstrict` value.
	let b = jstrict::Value::from_sonic_rs(a);

	// We convert it back into a `sonic_rs` value.
	let _ = jstrict::Value::into_sonic_rs(b);
}
